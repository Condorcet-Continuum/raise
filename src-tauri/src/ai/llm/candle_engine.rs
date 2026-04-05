// FICHIER : src-tauri/src/ai/llm/candle_engine.rs

use crate::utils::prelude::*;

use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_qwen2 as model;
use tokenizers::Tokenizer;

pub struct CandleLlmEngine {
    model: model::ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    logits_processor: LogitsProcessor,
}

impl CandleLlmEngine {
    pub async fn new(
        manager: &crate::json_db::collections::manager::CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        // 1. Récupération stricte de la configuration dynamique
        let (model_filename, tokenizer_filename) = match AppConfig::get_llm_settings(manager).await
        {
            Ok(settings) => settings,
            Err(e) => raise_error!(
                "ERR_AI_ENGINE_CONFIG_FAILED",
                error = "Impossible de charger la configuration LLM via la base de données.",
                context = json_value!({"source": e.to_string()})
            ),
        };

        // 2. Construction des chemins via la configuration métier (et non le système OS direct)
        let config = AppConfig::get();
        let base_path = match config.get_path("PATH_RAISE_DOMAIN") {
            Some(p) => p.join("_system/ai-assets/models"),
            None => raise_error!(
                "ERR_CONFIG_DOMAIN_PATH_MISSING",
                error =
                    "Le chemin racine 'PATH_RAISE_DOMAIN' n'est pas défini dans la configuration."
            ),
        };

        let model_path = base_path.join(&model_filename);
        let tokenizer_path = base_path.join(&tokenizer_filename);

        // 3. Vérifications de sécurité avec Pattern Matching exhaustif
        match model_path.exists() {
            true => {}
            false => raise_error!(
                "ERR_AI_MODEL_FILE_NOT_FOUND",
                error = format!(
                    "Le fichier GGUF configuré est introuvable : {:?}",
                    model_path
                ),
                context = json_value!({ "path": model_path.to_string_lossy(), "model_name": model_filename })
            ),
        }

        match tokenizer_path.exists() {
            true => {}
            false => raise_error!(
                "ERR_AI_TOKENIZER_FILE_NOT_FOUND",
                error = format!(
                    "Le fichier Tokenizer configuré est introuvable : {:?}",
                    tokenizer_path
                ),
                context = json_value!({ "path": tokenizer_path.to_string_lossy(), "tokenizer_name": tokenizer_filename })
            ),
        }

        let device = AppConfig::device().clone();
        tracing::info!(
            "🚀 [Candle LLM] Moteur Qwen ({}) chargé sur : {:?}",
            model_filename,
            device
        );

        // 4. Chargement du Tokenizer
        let tokenizer = match tokenizers::Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_AI_TOKENIZER_LOAD_FAILED",
                error = e,
                context = json_value!({ "path": tokenizer_path.to_string_lossy() })
            ),
        };

        // 5. Chargement du modèle GGUF
        let mut file = match fs::open_sync(&model_path) {
            Ok(f) => f,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_OPEN",
                error = e,
                context = json_value!({ "path": model_path.to_string_lossy() })
            ),
        };

        let model_content = match gguf_file::Content::read(&mut file) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_READ_CONTENT",
                error = e,
                context = json_value!({ "path": model_path.to_string_lossy() })
            ),
        };

        // 6. Initialisation de l'architecture Qwen2
        let weights = match model::ModelWeights::from_gguf(model_content, &mut file, &device) {
            Ok(w) => w,
            Err(e) => raise_error!(
                "ERR_AI_QWEN2_WEIGHTS_LOAD",
                error = e,
                context = json_value!({ "path": model_path.to_string_lossy() })
            ),
        };

        let logits_processor = LogitsProcessor::new(299792458, Some(0.7), None);

        Ok(Self {
            model: weights,
            tokenizer,
            device,
            logits_processor,
        })
    }

    fn format_prompt(system_prompt: &str, user_prompt: &str) -> String {
        format!(
            "<|im_start|>system\n{}<|im_end|>\n<|im_start|>user\n{}<|im_end|>\n<|im_start|>assistant\n",
            system_prompt, user_prompt
        )
    }

    pub fn generate(
        &mut self,
        system_prompt: &str,
        user_prompt: &str,
        max_tokens: usize,
    ) -> RaiseResult<String> {
        let formatted_prompt = Self::format_prompt(system_prompt, user_prompt);

        let tokens = match self.tokenizer.encode(formatted_prompt.as_str(), true) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_TOKENIZER_ENCODE",
                error = e,
                context = json_value!({ "prompt_preview": formatted_prompt.chars().take(50).collect::<String>() })
            ),
        };

        let mut tokens = tokens.get_ids().to_vec();
        let mut generated_tokens = Vec::new();
        let mut index_pos = 0;

        // 🎯 FIN DE LA MAGIE NOIRE : Résolution dynamique des IDs de stop tokens !
        let eos_token_id = match self.tokenizer.token_to_id("<|im_end|>") {
            Some(id) => id,
            None => raise_error!(
                "ERR_AI_MISSING_EOS_TOKEN",
                error = "Le token <|im_end|> est absent du tokenizer. Ce modèle n'est pas formaté pour ChatML."
            )
        };

        let stop_token_id = match self.tokenizer.token_to_id("<|endoftext|>") {
            Some(id) => id,
            None => raise_error!(
                "ERR_AI_MISSING_STOP_TOKEN",
                error = "Le token <|endoftext|> est absent du tokenizer."
            ),
        };

        for _i in 0..max_tokens {
            let context_size = if index_pos == 0 { tokens.len() } else { 1 };
            let start_pos = tokens.len().saturating_sub(context_size);

            let input = match Tensor::new(&tokens[start_pos..], &self.device) {
                Ok(t) => t,
                Err(e) => raise_error!(
                    "ERR_AI_TENSOR_CREATION_FAILED",
                    error = e,
                    context = json_value!({ "pos": index_pos })
                ),
            };

            let input = match input.unsqueeze(0) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_AI_TENSOR_SHAPE_ERROR", error = e),
            };

            let logits = match self.model.forward(&input, index_pos) {
                Ok(l) => l,
                Err(e) => raise_error!(
                    "ERR_AI_FORWARD_PASS_FAILED",
                    error = e,
                    context = json_value!({ "index_pos": index_pos })
                ),
            };

            let logits = match logits.squeeze(0) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_AI_TENSOR_SQUEEZE_FAILED", error = e),
            };

            let logits = match logits.squeeze(0) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_AI_TENSOR_SQUEEZE_FAILED", error = e),
            };

            let logits = match logits.to_dtype(candle_core::DType::F32) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_AI_DTYPE_CONVERSION_FAILED", error = e),
            };

            let next_token = match self.logits_processor.sample(&logits) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_AI_SAMPLING_FAILED", error = e),
            };

            if next_token == eos_token_id || next_token == stop_token_id {
                break;
            }

            tokens.push(next_token);
            generated_tokens.push(next_token);
            index_pos += context_size;
        }

        let result = match self.tokenizer.decode(&generated_tokens, true) {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_TOKENIZER_DECODE",
                error = e,
                context = json_value!({ "token_count": generated_tokens.len() })
            ),
        };
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[test]
    fn test_qwen_chatml_format() {
        let sys = "Sys";
        let user = "User";
        let expected = "<|im_start|>system\nSys<|im_end|>\n<|im_start|>user\nUser<|im_end|>\n<|im_start|>assistant\n";
        assert_eq!(CandleLlmEngine::format_prompt(sys, user), expected);
    }

    #[async_test]
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_quick_inference() {
        let sandbox = AgentDbSandbox::new().await;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // Appel magique de ton Mock Helper !
        inject_mock_component(&manager, "llm", json_value!({})).await;

        // On passe le manager au moteur
        let mut engine = CandleLlmEngine::new(&manager)
            .await
            .expect("Echec Init Engine");

        println!("🤖 Generating...");
        let response = engine
            .generate("Réponds 'OK'.", "Tu m'entends ?", 10)
            .expect("Echec Generation");

        println!("📝 Réponse: {}", response);
        assert!(!response.is_empty());
    }
}
