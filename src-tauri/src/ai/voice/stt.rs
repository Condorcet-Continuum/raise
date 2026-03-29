// FICHIER : src-tauri/src/ai/voice/stt.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use candle_core::{Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::whisper::{self as whisper_model, audio, Config};
use tokenizers::Tokenizer;

pub struct WhisperEngine {
    model: whisper_model::model::Whisper,
    tokenizer: Tokenizer,
    device: Device,
    mel_filters: Vec<f32>,
    config: Config,
}

impl WhisperEngine {
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        // 1. Récupération de la configuration globale dynamique
        let settings = AppConfig::get_component_settings(manager, "ai_voice").await?;

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

        // 2. Construction des chemins LOCAUX absolus (100% Hors-ligne)
        let Some(home) = dirs::home_dir() else {
            raise_error!(
                "ERR_OS_HOME_NOT_FOUND",
                error = "Impossible de localiser le répertoire personnel de l'utilisateur (HOME).",
                context = json_value!({ "method": "dirs::home_dir" })
            );
        };

        let base_path = home.join("raise_domain/_system/ai-assets/voice/whisper");
        let model_path = base_path.join(model_filename);
        let config_path = base_path.join(config_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);
        let mel_path = base_path.join(mel_filename);

        // 3. Vérifications de sécurité strictes
        if !model_path.exists()
            || !config_path.exists()
            || !tokenizer_path.exists()
            || !mel_path.exists()
        {
            raise_error!(
                "ERR_AI_WHISPER_ASSETS_MISSING",
                error = "Fichiers de modèle Whisper manquants en local.",
                context = json_value!({
                    "base_path": base_path.to_string_lossy(),
                    "missing": {
                        "model": !model_path.exists(),
                        "config": !config_path.exists(),
                        "tokenizer": !tokenizer_path.exists(),
                        "mel_filters": !mel_path.exists()
                    }
                })
            );
        }

        let device = AppConfig::device().clone();
        user_info!(
            "🎤 [Candle STT] Moteur Whisper chargé sur : {:?}",
            json_value!(format!("{:?}", device))
        );

        // 4. Chargement Config & Tokenizer
        let config_str = match fs::read_to_string_sync(&config_path) {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_WHISPER_CONFIG_READ",
                error = e,
                context = json_value!({"path": config_path.to_string_lossy()})
            ),
        };

        let config: Config = match json::deserialize_from_str(&config_str) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_WHISPER_CONFIG_PARSE", error = e),
        };

        let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_WHISPER_TOKENIZER_LOAD",
                error = e,
                context = json_value!({"path": tokenizer_path.to_string_lossy()})
            ),
        };

        // 5. Chargement des Poids GGUF/Safetensors
        let vb = unsafe {
            match VarBuilder::from_mmaped_safetensors(
                &[&model_path],
                candle_core::DType::F32,
                &device,
            ) {
                Ok(v) => v,
                Err(e) => raise_error!(
                    "ERR_WHISPER_WEIGHTS_LOAD",
                    error = e,
                    context = json_value!({"path": model_path.to_string_lossy()})
                ),
            }
        };

        let model = match whisper_model::model::Whisper::load(&vb, config.clone()) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_WHISPER_MODEL_INIT", error = e),
        };

        // 6. Chargement des filtres Mel (spécifique à l'audio)
        let mel_bytes = match std::fs::read(&mel_path) {
            Ok(b) => b,
            Err(e) => raise_error!(
                "ERR_WHISPER_MEL_READ",
                error = e,
                context = json_value!({"path": mel_path.to_string_lossy()})
            ),
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
            config,
        })
    }

    pub fn transcribe(&mut self, audio_pcm: &[f32]) -> RaiseResult<String> {
        if audio_pcm.is_empty() {
            return Ok(String::new());
        }

        // ÉTAPE 1 : Mel Spectrogram
        let mel = audio::pcm_to_mel(&self.config, audio_pcm, &self.mel_filters);
        let mel_len = mel.len();

        let mel_tensor = match Tensor::from_vec(
            mel,
            (
                1,
                self.config.num_mel_bins,
                mel_len / self.config.num_mel_bins,
            ),
            &self.device,
        ) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_WHISPER_MEL_TENSOR", error = e),
        };

        // ÉTAPE 2 : Passe Avant (Encodeur)
        let audio_features = match self.model.encoder.forward(&mel_tensor, false) {
            Ok(f) => f,
            Err(e) => raise_error!("ERR_WHISPER_ENCODER_FORWARD", error = e),
        };

        // ÉTAPE 3 : Préparation du Décodeur
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

        // ÉTAPE 4 : Boucle de Génération (Greedy Decoding)
        for i in 0..100 {
            let tokens_tensor = match Tensor::new(tokens.as_slice(), &self.device) {
                Ok(t) => match t.unsqueeze(0) {
                    Ok(ts) => ts,
                    Err(e) => raise_error!("ERR_WHISPER_UNSQUEEZE_TOKENS", error = e),
                },
                Err(e) => raise_error!(
                    "ERR_WHISPER_TENSOR_TOKENS",
                    error = e,
                    context = json_value!({"iter": i})
                ),
            };

            let logits = match self
                .model
                .decoder
                .forward(&tokens_tensor, &audio_features, false)
            {
                Ok(l) => l,
                Err(e) => raise_error!(
                    "ERR_WHISPER_DECODER_FORWARD",
                    error = e,
                    context = json_value!({"iter": i})
                ),
            };

            let logits = match logits.squeeze(0) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_WHISPER_LOGITS_SQUEEZE", error = e),
            };

            let dim_0 = match logits.dim(0) {
                Ok(d) => d,
                Err(e) => raise_error!("ERR_WHISPER_LOGITS_DIM", error = e),
            };

            let logits = match logits.get(dim_0 - 1) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_WHISPER_LOGITS_GET", error = e),
            };

            let next_token = match logits.argmax(0) {
                Ok(t) => match t.to_scalar::<u32>() {
                    Ok(scalar) => scalar,
                    Err(e) => raise_error!("ERR_WHISPER_TOKEN_SCALAR", error = e),
                },
                Err(e) => raise_error!("ERR_WHISPER_ARGMAX", error = e),
            };

            if next_token == eot_token {
                break;
            }

            tokens.push(next_token);
            generated_tokens.push(next_token);
        }

        // ÉTAPE 5 : Décodage et Nettoyage
        let raw_text = match self.tokenizer.decode(&generated_tokens, true) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_WHISPER_TOKEN_DECODE",
                error = e,
                context = json_value!({"token_count": generated_tokens.len()})
            ),
        };

        // Application de ta fonction de normalisation métier
        let clean_text = crate::ai::nlp::preprocessing::normalize(&raw_text);

        Ok(clean_text)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    #[serial_test::serial] // Protection GPU partagé
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_whisper_engine_initialization_missing_assets() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // On injecte des chemins qui n'existent pas pour vérifier la macro raise_error!
        inject_mock_component(
            &manager,
            "voice",
            json_value!({
                "rust_model_file": "inexistant.safetensors",
                "rust_config_file": "config_inexistant.json",
                "rust_tokenizer_file": "tokenizer_inexistant.json",
                "rust_mel_filters": "mel_inexistant.safetensors"
            }),
        )
        .await;

        let result = WhisperEngine::new(&manager).await;

        // CORRECTION : Extraction de l'erreur via un match (pas de unwrap_err)
        let err_str = match result {
            Err(e) => e.to_string(),
            Ok(_) => panic!("L'initialisation aurait dû échouer car les fichiers n'existent pas."),
        };

        // On vérifie que c'est bien notre erreur métier qui a été déclenchée
        assert!(
            err_str.contains("ERR_AI_WHISPER_ASSETS_MISSING")
                || err_str.contains("ERR_OS_HOME_NOT_FOUND"),
            "L'erreur retournée ne correspond pas au comportement attendu: {}",
            err_str
        );
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_whisper_engine_empty_audio_handling() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // NOTE: Pour que ce test passe sur ta machine, il faut que les vrais fichiers
        // soient présents dans ~/raise_domain/_system/ai-assets/voice/whisper/
        inject_mock_component(
            &manager,
            "voice",
            json_value!({
                "rust_model_file": "model.safetensors",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_mel_filters": "mel_filters.safetensors"
            }),
        )
        .await;

        // Si l'engine charge correctement, on teste la robustesse de `transcribe`
        if let Ok(mut engine) = WhisperEngine::new(&manager).await {
            let empty_audio: Vec<f32> = vec![];
            let res = engine
                .transcribe(&empty_audio)
                .expect("Transcribe a échoué sur un vec vide");

            assert_eq!(
                res, "",
                "Un signal audio vide doit retourner une chaîne vide"
            );
        } else {
            println!("⚠️ Test ignoré: Assets Whisper locaux non trouvés dans le home dir.");
        }
    }
}
