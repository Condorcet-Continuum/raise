// FICHIER : crates/raise-core/src/services/voice_service.rs

use crate::ai::voice::stt::WhisperEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use crate::utils::io::audio::AudioListener;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

/// État partagé pour gérer la session vocale asynchrone.
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

/// Gère le démarrage/arrêt du micro et l'init de Whisper.
/// Retourne un booléen (état) et optionnellement le Receiver du flux audio.
pub async fn toggle_voice_assistant(
    voice_state: &VoiceState,
) -> RaiseResult<(bool, Option<tokio::sync::mpsc::Receiver<Vec<f32>>>)> {
    let mut is_listening = voice_state.is_listening.lock().await;

    // ==========================================
    // CAS 1 : ARRÊT DU MICROPHONE
    // ==========================================
    if *is_listening {
        *is_listening = false;

        let mut listener_guard = match voice_state._listener.lock() {
            Ok(g) => g,
            Err(_) => raise_error!("ERR_VOICE_MUTEX_POISONED", error = "Verrou audio corrompu"),
        };

        *listener_guard = None; // Coupe proprement le flux natif cpal
        user_info!("🛑 [Voice] Microphone désactivé.", json_value!({}));
        return Ok((false, None));
    }

    // ==========================================
    // CAS 2 : DÉMARRAGE DE L'ASSISTANT
    // ==========================================
    user_info!(
        "⏳ [Voice] Initialisation de l'assistant...",
        json_value!({})
    );

    // Étape A : Chargement résilient
    let mut engine_guard = voice_state.engine.lock().await;
    if engine_guard.is_none() {
        let config = AppConfig::get();
        let storage = StorageEngine::new(JsonDbConfig::new(PathBuf::from(
            &config.mount_points.system.db,
        )))?;
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
    let (listener, rx) = match AudioListener::start() {
        Ok(res) => res,
        Err(e) => raise_error!("ERR_VOICE_AUDIO_START", error = e.to_string()),
    };

    let mut listener_guard = match voice_state._listener.lock() {
        Ok(g) => g,
        Err(_) => raise_error!("ERR_VOICE_MUTEX_POISONED", error = "Verrou audio corrompu"),
    };

    *listener_guard = Some(listener);
    *is_listening = true;

    Ok((true, Some(rx)))
}

/// Fonction pure pour transcrire un bloc audio
pub async fn transcribe_audio(
    voice_state: &VoiceState,
    audio_chunk: &[f32],
) -> RaiseResult<String> {
    let mut engine_guard = voice_state.engine.lock().await;
    if let Some(engine) = engine_guard.as_mut() {
        match engine.transcribe(audio_chunk) {
            Ok(t) => Ok(t),
            Err(e) => raise_error!("ERR_VOICE_STT", error = e.to_string()),
        }
    } else {
        raise_error!("ERR_VOICE_ENGINE_NOT_READY", error = "Moteur STT inactif")
    }
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
