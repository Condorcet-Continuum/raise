// FICHIER : src-tauri/src/commands/voice_commands.rs

use crate::ai::voice::stt::WhisperEngine;
use crate::commands::ai_commands::AiState;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use crate::utils::io::audio::AudioListener;
use crate::utils::prelude::*;

use tauri::{command, AppHandle, Emitter, Manager, State};

/// État partagé pour gérer la session vocale asynchrone
pub struct VoiceState {
    pub engine: AsyncMutex<Option<WhisperEngine>>,
    pub is_listening: AsyncMutex<bool>,
    // On garde le listener natif dans un Mutex standard car cpal gère son propre thread OS
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

/// Commande Tauri : Active ou désactive l'assistant vocal ("Toggle")
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
                error = "Listener mutex poisoned"
            ),
        };

        // Le fait de remplacer par None "Drop" l'instance AudioListener
        // et coupe donc proprement le flux natif cpal.
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

    // Étape A : Charger le moteur Whisper s'il n'est pas encore en mémoire
    let mut engine_guard = voice_state.engine.lock().await;
    if engine_guard.is_none() {
        let config = AppConfig::get();
        let path_buf = PathBuf::from(&config.system_db);
        let db_config = JsonDbConfig::new(path_buf);
        let storage = StorageEngine::new(db_config);
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        let engine = WhisperEngine::new(&manager).await?;
        *engine_guard = Some(engine);
    }
    // Libération explicite du lock pour que la boucle d'écoute puisse l'utiliser
    drop(engine_guard);

    // Étape B : Démarrer l'écoute physique via cpal
    let (listener, mut rx) = AudioListener::start()?;

    let mut listener_guard = match voice_state._listener.lock() {
        Ok(g) => g,
        Err(_) => raise_error!(
            "ERR_VOICE_MUTEX_POISONED",
            error = "Listener mutex poisoned"
        ),
    };
    *listener_guard = Some(listener);
    *is_listening = true;

    let _ = app.emit("voice_status_changed", json_value!({"status": "listening"}));

    // Étape C : Lancer la boucle de traitement en tâche de fond (Tokio)
    let app_clone = app.clone();

    tokio::spawn(async move {
        // Boucle sur les "chunks" audio détectés par le VAD
        while let Some(audio_chunk) = rx.recv().await {
            // On vérifie si l'utilisateur a coupé le micro entre-temps
            let state = app_clone.state::<VoiceState>();
            if !*state.is_listening.lock().await {
                break;
            }

            let _ = app_clone.emit(
                "voice_status_changed",
                json_value!({"status": "transcribing"}),
            );

            // Étape D : STT (Voix -> Texte)
            let text = {
                let mut e_guard = state.engine.lock().await;
                if let Some(engine) = e_guard.as_mut() {
                    match engine.transcribe(&audio_chunk) {
                        Ok(t) => t,
                        Err(e) => {
                            eprintln!("❌ [Voice] Erreur STT : {}", e);
                            let _ = app_clone.emit(
                                "voice_error",
                                json_value!({"error": "Erreur de transcription locale"}),
                            );
                            continue;
                        }
                    }
                } else {
                    continue;
                }
            };

            // On ignore les silences ou bruits de fond transcrits comme vides
            if text.trim().is_empty() {
                let _ =
                    app_clone.emit("voice_status_changed", json_value!({"status": "listening"}));
                continue;
            }

            user_info!("🗣️ [Voice] Compris : {}", json_value!(text.clone()));
            let _ = app_clone.emit("voice_transcription_result", json_value!({"text": &text}));
            let _ = app_clone.emit("voice_status_changed", json_value!({"status": "thinking"}));

            // Étape E : NLP & Routage vers l'Orchestrateur (Ton code existant)
            let ai_state = app_clone.state::<AiState>();
            let ai_guard = ai_state.0.lock().await;

            if let Some(shared_orch) = &*ai_guard {
                let mut orchestrator = shared_orch.lock().await;

                // Appel exact au pipeline de l'Orchestrateur (qui appelle ton IntentClassifier)
                match orchestrator.execute_workflow(&text).await {
                    Ok(res) => {
                        user_success!("✅ [Voice] Action exécutée !", json_value!({}));
                        let _ = app_clone.emit("voice_workflow_result", res);
                    }
                    Err(e) => {
                        eprintln!("❌ [Voice] Erreur Orchestrateur : {}", e);
                        let _ =
                            app_clone.emit("voice_error", json_value!({"error": e.to_string()}));
                    }
                }
            } else {
                eprintln!("⚠️ [Voice] Orchestrateur non initialisé.");
            }

            // Retour à l'état d'écoute
            let _ = app_clone.emit("voice_status_changed", json_value!({"status": "listening"}));
        }
    });

    Ok("Assistant vocal démarré avec succès.".to_string())
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_voice_state_defaults() {
        let state = VoiceState::default();

        // 1. L'assistant ne doit pas écouter par défaut
        let is_listening = state.is_listening.lock().await;
        assert_eq!(
            *is_listening, false,
            "L'état d'écoute doit être faux par défaut"
        );

        // 2. Le moteur lourd (Whisper) ne doit pas être chargé au démarrage pour sauver la RAM/VRAM
        let engine = state.engine.lock().await;
        assert!(
            engine.is_none(),
            "Le moteur STT ne doit pas être alloué au démarrage"
        );

        // 3. Le microphone ne doit pas être capturé
        let listener = state._listener.lock().unwrap();
        assert!(
            listener.is_none(),
            "Aucun flux audio ne doit être ouvert au démarrage"
        );
    }

    #[async_test]
    async fn test_voice_state_toggle_simulation_no_deadlocks() {
        let state = VoiceState::new();

        // --- SIMULATION : L'utilisateur clique sur "Activer la Voix" ---
        {
            let mut is_listening = state.is_listening.lock().await;
            *is_listening = true;

            // On simule l'allocation d'un listener natif (sans lancer vraiment cpal)
            let listener_guard = state
                ._listener
                .lock()
                .expect("Le Mutex Synchrone a été empoisonné !");
            // En vrai on ferait : *listener_guard = Some(AudioListener::start().unwrap());
            assert!(listener_guard.is_none());
        } // Les locks sont relâchés ici grâce au Drop de la portée.

        // Vérification de l'état intermédiaire
        assert_eq!(*state.is_listening.lock().await, true);

        // --- SIMULATION : L'utilisateur clique sur "Désactiver la Voix" ---
        {
            let mut is_listening = state.is_listening.lock().await;
            *is_listening = false;

            let mut listener_guard = state
                ._listener
                .lock()
                .expect("Le Mutex Synchrone a été empoisonné !");
            *listener_guard = None; // Cela provoque le drop() de l'AudioListener en production
        }

        // Vérification de l'état final
        assert_eq!(*state.is_listening.lock().await, false);
        assert!(state._listener.lock().unwrap().is_none());
    }
}
