// FICHIER : src-tauri/src/commands/voice_commands.rs

use crate::ai::voice::stt::WhisperEngine;
use crate::commands::ai_commands::AiState;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use crate::utils::io::audio::AudioListener;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use tauri::{command, AppHandle, Emitter, Manager, State};

/// État partagé pour gérer la session vocale asynchrone.
/// Utilise des Mutex asynchrones pour les moteurs et synchrones pour les threads audio OS.
pub struct VoiceState {
    pub engine: AsyncMutex<Option<WhisperEngine>>,
    pub is_listening: AsyncMutex<bool>,
    pub _listener: SyncMutex<Option<AudioListener>>,
}

impl VoiceState {
    pub fn new() -> Self {
        Self {
            engine: AsyncMutex::new(None),
            is_listening: AsyncMutex::new(false),
            _listener: SyncMutex::new(None),
        }
    }
}

impl Default for VoiceState {
    fn default() -> Self {
        Self::new()
    }
}

/// Commande Tauri : Active ou désactive l'assistant vocal ("Toggle").
/// Gère la résilience des points de montage et l'orchestration multi-agents.
#[command]
pub async fn toggle_voice_assistant(
    app: AppHandle,
    voice_state: State<'_, VoiceState>,
) -> RaiseResult<String> {
    let mut is_listening = voice_state.is_listening.lock().await;

    // ==========================================
    // CAS 1 : ARRÊT DU MICROPHONE
    // ==========================================
    if *is_listening {
        *is_listening = false;

        let mut listener_guard = match voice_state._listener.lock() {
            Ok(g) => g,
            Err(_) => raise_error!(
                "ERR_VOICE_MUTEX_POISONED",
                error = "Le verrou du listener audio est corrompu."
            ),
        };

        // Le Drop de l'AudioListener coupe proprement le flux natif cpal
        *listener_guard = None;

        user_info!("🛑 [Voice] Microphone désactivé.", json_value!({}));
        let _ = app.emit("voice_status_changed", json_value!({"status": "idle"}));

        return Ok("Assistant vocal désactivé.".to_string());
    }

    // ==========================================
    // CAS 2 : DÉMARRAGE DE L'ASSISTANT
    // ==========================================
    user_info!(
        "⏳ [Voice] Initialisation de l'assistant...",
        json_value!({})
    );
    let _ = app.emit("voice_status_changed", json_value!({"status": "loading"}));

    // Étape A : Chargement résilient via Mount Points (System Domain)
    let mut engine_guard = voice_state.engine.lock().await;
    if engine_guard.is_none() {
        let config = AppConfig::get();

        // Résolution dynamique du stockage via la partition système configurée
        let storage = StorageEngine::new(JsonDbConfig::new(PathBuf::from(
            &config.mount_points.system.db,
        )));
        let manager = CollectionsManager::new(
            &storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        match WhisperEngine::new(&manager).await {
            Ok(engine) => *engine_guard = Some(engine),
            Err(e) => raise_error!("ERR_VOICE_ENGINE_INIT", error = e.to_string()),
        }
    }
    drop(engine_guard);

    // Étape B : Démarrer l'écoute physique
    let (listener, mut rx) = match AudioListener::start() {
        Ok(res) => res,
        Err(e) => raise_error!("ERR_VOICE_AUDIO_START", error = e.to_string()),
    };

    let mut listener_guard = match voice_state._listener.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_VOICE_MUTEX_POISONED"),
    };

    *listener_guard = Some(listener);
    *is_listening = true;

    let _ = app.emit("voice_status_changed", json_value!({"status": "listening"}));

    // Étape C : Boucle de traitement asynchrone (STT -> Orchestrateur)
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(audio_chunk) = rx.recv().await {
            let state = app_clone.state::<VoiceState>();
            if !*state.is_listening.lock().await {
                break;
            }

            let _ = app_clone.emit(
                "voice_status_changed",
                json_value!({"status": "transcribing"}),
            );

            // Étape D : Transcription (STT)
            let text = {
                let mut e_guard = state.engine.lock().await;
                match e_guard.as_mut() {
                    Some(engine) => match engine.transcribe(&audio_chunk) {
                        Ok(t) => t,
                        Err(e) => {
                            user_error!("ERR_VOICE_STT", json_value!({"error": e.to_string()}));
                            continue;
                        }
                    },
                    None => continue,
                }
            };

            if text.trim().is_empty() {
                let _ =
                    app_clone.emit("voice_status_changed", json_value!({"status": "listening"}));
                continue;
            }

            user_info!("🗣️ [Voice] Compris : {}", json_value!(text.clone()));
            let _ = app_clone.emit("voice_transcription_result", json_value!({"text": &text}));
            let _ = app_clone.emit("voice_status_changed", json_value!({"status": "thinking"}));

            // Étape E : Routage vers l'Orchestrateur multi-agents
            let ai_state = app_clone.state::<AiState>();
            let ai_guard = ai_state.0.lock().await;

            if let Some(shared_orch) = &*ai_guard {
                let mut orchestrator = shared_orch.lock().await;
                match orchestrator.execute_workflow(&text).await {
                    Ok(res) => {
                        user_success!("✅ [Voice] Action exécutée !", json_value!({}));
                        let _ = app_clone.emit("voice_workflow_result", res);
                    }
                    Err(e) => {
                        user_error!("ERR_VOICE_WORKFLOW", json_value!({"error": e.to_string()}));
                        let _ =
                            app_clone.emit("voice_error", json_value!({"error": e.to_string()}));
                    }
                }
            }
            let _ = app_clone.emit("voice_status_changed", json_value!({"status": "listening"}));
        }
    });

    Ok("Assistant vocal démarré avec succès.".to_string())
}

// =========================================================================
// TESTS UNITAIRES (Respect des tests existants & Résilience Mount Points)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_voice_state_defaults() -> RaiseResult<()> {
        let state = VoiceState::default();

        let is_listening = state.is_listening.lock().await;
        assert_eq!(*is_listening, false);

        let engine = state.engine.lock().await;
        assert!(engine.is_none());

        let listener = match state._listener.lock() {
            Ok(g) => g,
            Err(_) => panic!("Poisoned"),
        };
        assert!(listener.is_none());
        Ok(())
    }

    #[async_test]
    async fn test_voice_state_toggle_simulation_no_deadlocks() -> RaiseResult<()> {
        let state = VoiceState::new();

        // Simulation : Activation
        {
            let mut is_listening = state.is_listening.lock().await;
            *is_listening = true;

            let listener_guard = state._listener.lock().expect("Poisoned");
            assert!(listener_guard.is_none());
        }

        assert_eq!(*state.is_listening.lock().await, true);

        // Simulation : Désactivation
        {
            let mut is_listening = state.is_listening.lock().await;
            *is_listening = false;

            let mut listener_guard = state._listener.lock().expect("Poisoned");
            *listener_guard = None;
        }

        assert_eq!(*state.is_listening.lock().await, false);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience des points de montage (Mount Points)
    #[async_test]
    async fn test_voice_mount_point_resolution() -> RaiseResult<()> {
        let config = AppConfig::get();

        // On vérifie que les chemins dynamiques sont bien résolus pour Whisper
        assert!(!config.mount_points.system.domain.is_empty());
        assert!(!config.mount_points.system.db.is_empty());

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face au verrouillage (Mutex Match)
    #[async_test]
    async fn test_voice_mutex_resilience() -> RaiseResult<()> {
        let state = VoiceState::new();
        // Vérification du pattern match sur le verrou synchrone utilisé par toggle
        let guard_res = state._listener.lock();
        assert!(guard_res.is_ok());
        Ok(())
    }
}
