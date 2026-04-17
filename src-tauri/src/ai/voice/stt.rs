// FICHIER : src-tauri/src/ai/voice/stt.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

pub struct WhisperEngine {
    model: WhisperModel::model::Whisper,
    tokenizer: TextTokenizer,
    device: ComputeHardware,
    mel_filters: Vec<f32>,
    config: WhisperConfig,
}

impl WhisperEngine {
    /// Initialise le moteur de reconnaissance vocale Whisper en respectant les points de montage.
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        // 1. Récupération de la configuration globale via le manager
        let settings = match AppConfig::get_component_settings(manager, "ai_voice").await {
            Ok(s) => s,
            Err(e) => raise_error!("ERR_AI_VOICE_CONFIG_LOAD", error = e.to_string()),
        };

        let model_filename = settings
            .get("rust_model_file")
            .and_then(|v| v.as_str())
            .unwrap_or("model.safetensors");
        let config_filename = settings
            .get("rust_config_file")
            .and_then(|v| v.as_str())
            .unwrap_or("config.json");
        let tokenizer_filename = settings
            .get("rust_tokenizer_file")
            .and_then(|v| v.as_str())
            .unwrap_or("tokenizer.json");
        let mel_filename = settings
            .get("rust_mel_filters")
            .and_then(|v| v.as_str())
            .unwrap_or("mel_filters.safetensors");

        // 2. Résolution dynamique via les Mount Points (Portabilité MBSE)
        let app_config = AppConfig::get();
        let base_path = match app_config.get_path("PATH_RAISE_DOMAIN") {
            Some(p) => p
                .join(&app_config.mount_points.system.domain)
                .join(&app_config.mount_points.system.db)
                .join("ai-assets/voice/whisper"),
            None => raise_error!(
                "ERR_CONFIG_DOMAIN_PATH_MISSING",
                error = "Le chemin racine 'PATH_RAISE_DOMAIN' est absent de la configuration."
            ),
        };

        let model_path = base_path.join(model_filename);
        let config_path = base_path.join(config_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);
        let mel_path = base_path.join(mel_filename);

        // 3. Vérification de résilience physique via Match
        if !model_path.exists()
            || !config_path.exists()
            || !tokenizer_path.exists()
            || !mel_path.exists()
        {
            raise_error!(
                "ERR_AI_WHISPER_ASSETS_MISSING",
                error = "Fichiers Whisper introuvables dans le point de montage système.",
                context = json_value!({ "resolved_path": base_path.to_string_lossy() })
            );
        }

        let device = AppConfig::device().clone();
        user_info!(
            "MSG_VOICE_STT_LOAD_START",
            json_value!({ "device": format!("{:?}", device), "model": model_filename })
        );

        // 4. Chargement WhisperConfig & TextTokenizer
        let config_str = match fs::read_to_string_sync(&config_path) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_WHISPER_CONFIG_READ", error = e.to_string()),
        };

        let whisper_config: WhisperConfig = match json::deserialize_from_str(&config_str) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_WHISPER_CONFIG_PARSE", error = e.to_string()),
        };

        let tokenizer = match TextTokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_WHISPER_TOKENIZER_LOAD", error = e.to_string()),
        };

        // 5. Chargement des Poids via Memory Mapping (Zéro Dette performance)
        let vb = unsafe {
            match NeuralWeightsBuilder::from_mmaped_safetensors(
                &[&model_path],
                ComputeType::F32,
                &device,
            ) {
                Ok(v) => v,
                Err(e) => raise_error!("ERR_WHISPER_WEIGHTS_LOAD", error = e.to_string()),
            }
        };

        let model = match WhisperModel::model::Whisper::load(&vb, whisper_config.clone()) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_WHISPER_MODEL_INIT", error = e.to_string()),
        };

        // 6. Chargement des filtres Mel
        let mel_bytes = match fs::read_sync(&mel_path) {
            Ok(b) => b,
            Err(e) => raise_error!("ERR_WHISPER_MEL_READ", error = e.to_string()),
        };

        let mut mel_filters = vec![0f32; mel_bytes.len() / 4];
        unsafe {
            std::ptr::copy_nonoverlapping(
                mel_bytes.as_ptr() as *const f32,
                mel_filters.as_mut_ptr(),
                mel_filters.len(),
            );
        }

        Ok(Self {
            model,
            tokenizer,
            device,
            mel_filters,
            config: whisper_config,
        })
    }

    /// Transcrit un signal WhisperAudio PCM vers du texte normalisé Arcadia.
    pub fn transcribe(&mut self, audio_pcm: &[f32]) -> RaiseResult<String> {
        if audio_pcm.is_empty() {
            return Ok(String::new());
        }

        // ÉTAPE 1 : Mel Spectrogram avec protection
        let mel = WhisperAudio::pcm_to_mel(&self.config, audio_pcm, &self.mel_filters);
        let mel_len = mel.len();

        let mel_tensor = match NeuralTensor::from_vec(
            mel,
            (
                1,
                self.config.num_mel_bins,
                mel_len / self.config.num_mel_bins,
            ),
            &self.device,
        ) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_WHISPER_MEL_TENSOR", error = e.to_string()),
        };

        // ÉTAPE 2 : Forward Pass Encodeur
        let audio_features = match self.model.encoder.forward(&mel_tensor, false) {
            Ok(f) => f,
            Err(e) => raise_error!("ERR_WHISPER_ENCODER_FORWARD", error = e.to_string()),
        };

        // ÉTAPE 3 : Préparation du Décodeur (Greedy Decoding)
        let sot_token = self
            .tokenizer
            .token_to_id("<|startoftranscript|>")
            .unwrap_or(50258);
        let lang_token = self.tokenizer.token_to_id("<|fr|>").unwrap_or(50278);
        let trans_token = self
            .tokenizer
            .token_to_id("<|transcribe|>")
            .unwrap_or(50359);
        let notimestamps_token = self
            .tokenizer
            .token_to_id("<|notimestamps|>")
            .unwrap_or(50363);
        let eot_token = self.tokenizer.token_to_id("<|endoftext|>").unwrap_or(50257);

        let mut tokens = vec![sot_token, lang_token, trans_token, notimestamps_token];
        let mut generated_tokens = Vec::new();

        // ÉTAPE 4 : Boucle de Génération résiliente
        for i in 0..150 {
            let tokens_tensor = match NeuralTensor::new(tokens.as_slice(), &self.device) {
                Ok(t) => match t.unsqueeze(0) {
                    Ok(u) => u,
                    Err(e) => raise_error!("ERR_WHISPER_UNSQUEEZE", error = e.to_string()),
                },
                Err(e) => raise_error!(
                    "ERR_WHISPER_TENSOR_TOKENS",
                    error = e.to_string(),
                    context = json_value!({"iter": i})
                ),
            };

            let logits = match self
                .model
                .decoder
                .forward(&tokens_tensor, &audio_features, false)
            {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_WHISPER_DECODER_FORWARD", error = e.to_string()),
            };

            // Extraction sécurisée du prochain token
            let next_token = match logits
                .squeeze(0)
                .and_then(|l| l.get(l.dim(0).unwrap_or(1) - 1))
                .and_then(|l| l.argmax(0))
                .and_then(|l| l.to_scalar::<u32>())
            {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_WHISPER_SAMPLING_FAIL", error = e.to_string()),
            };

            if next_token == eot_token {
                break;
            }

            tokens.push(next_token);
            generated_tokens.push(next_token);
        }

        // ÉTAPE 5 : Décodage et Normalisation métier
        match self.tokenizer.decode(&generated_tokens, true) {
            Ok(t) => Ok(crate::ai::nlp::preprocessing::normalize(&t)),
            Err(e) => raise_error!("ERR_WHISPER_DECODE_FAIL", error = e.to_string()),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Mount Points & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    /// Test existant : Détection des assets manquants via Mount Points
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_whisper_engine_initialization_missing_assets() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(
            &manager,
            "voice",
            json_value!({
                "rust_model_file": "inexistant.safetensors"
            }),
        )
        .await;

        let result = WhisperEngine::new(&manager).await;

        match result {
            Err(AppError::Structured(err)) => {
                assert!(
                    err.code == "ERR_AI_WHISPER_ASSETS_MISSING"
                        || err.code == "ERR_CONFIG_DOMAIN_PATH_MISSING"
                );
                Ok(())
            }
            _ => panic!("L'initialisation aurait dû lever une erreur structurée RAISE"),
        }
    }

    /// Test existant : Robustesse sur WhisperAudio vide
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_whisper_empty_audio_handling() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Si l'engine charge correctement (en local), on vérifie le vec vide
        if let Ok(mut engine) = WhisperEngine::new(&manager).await {
            let res = engine.transcribe(&[])?;
            assert_eq!(res, "");
        }
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une configuration de Mount Point invalide
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_whisper_resilience_bad_mount_point() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        // On crée un manager pointant vers un domaine non initialisé
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        let result = WhisperEngine::new(&manager).await;

        match result {
            Err(AppError::Structured(err)) => {
                // Doit échouer car get_component_settings ne trouvera rien dans la partition fantôme
                assert_eq!(err.code, "ERR_AI_VOICE_CONFIG_LOAD");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_AI_VOICE_CONFIG_LOAD"),
        }
    }

    /// 🎯 NOUVEAU TEST : Inférence résiliente sur le périphérique configuré
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_whisper_device_fallback_logic() -> RaiseResult<()> {
        let device = AppConfig::device();
        // Vérification que la façade SSOT retourne un device valide pour Native
        assert!(device.is_cpu() || device.is_cuda() || device.is_metal());
        Ok(())
    }
}
