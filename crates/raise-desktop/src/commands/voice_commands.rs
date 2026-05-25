// FICHIER : crates/raise-desktop/src/commands/voice_commands.rs

use raise_core::utils::prelude::*;

// 🎯 Imports purs depuis raise_core
use raise_core::services::ai_service::{self, AiState};
use raise_core::services::voice_service::{self, VoiceState};

use tauri::{command, AppHandle, Emitter, Manager, State};

#[command]
pub async fn toggle_voice_assistant(
    app: AppHandle,
    voice_state: State<'_, VoiceState>,
) -> RaiseResult<String> {
    let (is_listening, rx_opt) = voice_service::toggle_voice_assistant(voice_state.inner()).await?;

    // CAS 1 : On vient de l'éteindre
    if !is_listening {
        let _ = app.emit("voice_status_changed", json_value!({"status": "idle"}));
        return Ok("Assistant vocal désactivé.".to_string());
    }

    // CAS 2 : On vient de l'allumer
    let _ = app.emit("voice_status_changed", json_value!({"status": "listening"}));
    let mut rx = rx_opt.unwrap();
    let app_clone = app.clone();

    // 🚀 La boucle asynchrone appartient à l'interface (Elle orchestre les services)
    tokio::spawn(async move {
        while let Some(audio_chunk) = rx.recv().await {
            let state = app_clone.state::<VoiceState>();
            if !*state.is_listening.lock().await {
                break; // Le micro a été coupé entre temps
            }

            let _ = app_clone.emit(
                "voice_status_changed",
                json_value!({"status": "transcribing"}),
            );

            // 1. Transcription via le noyau
            let text = match voice_service::transcribe_audio(state.inner(), &audio_chunk).await {
                Ok(t) => t,
                Err(e) => {
                    user_error!("ERR_VOICE_STT", json_value!({"error": e.to_string()}));
                    continue;
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

            // 2. Routage vers l'IA (Orchestrateur) via le service métier IA
            let ai_state = app_clone.state::<AiState>();

            match ai_service::ai_chat(ai_state.inner(), &text).await {
                Ok(res) => {
                    user_success!("✅ [Voice] Action exécutée !", json_value!({}));
                    let _ = app_clone.emit("voice_workflow_result", res);
                }
                Err(e) => {
                    user_error!("ERR_VOICE_WORKFLOW", json_value!({"error": e.to_string()}));
                    let _ = app_clone.emit("voice_error", json_value!({"error": e.to_string()}));
                }
            }

            let _ = app_clone.emit("voice_status_changed", json_value!({"status": "listening"}));
        }
    });

    Ok("Assistant vocal démarré avec succès.".to_string())
}
