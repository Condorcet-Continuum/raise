// FICHIER : src-tauri/src/ai/llm/native_engine.rs

use crate::utils::prelude::*; // 🎯 Façade Unique

pub struct NativeTensorEngine {
    model: Qwen2QuantizedModel::ModelWeights,
    tokenizer: TextTokenizer,
    device: ComputeHardware,
    logits_processor: TokenLogitsProcessor,
}

impl NativeTensorEngine {
    /// Initialise le moteur LLM local en respectant les points de montage et la config dynamique.
    /// Initialise le moteur LLM local en respectant les points de montage et la config dynamique.
    pub async fn new(
        manager: &crate::json_db::collections::manager::CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        // 1. Récupération stricte de la configuration via la requête V2 (Zéro Dette)
        let svc_settings = match AppConfig::get_service_settings(
            manager,
            "ref:services:handle:svc_ai",
        )
        .await
        {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_AI_ENGINE_CONFIG_FAILED",
                error = e.to_string(),
                context = json_value!({"action": "fetch_service_settings", "service_id": "ref:services:handle:svc_ai"})
            ),
        };

        // 2. Extraction du composant spécifique LLM
        let comp_settings = match svc_settings.get("ref:components:handle:ai_llm") {
            Some(s) => s,
            None => raise_error!(
                "ERR_AI_LLM_COMPONENT_MISSING",
                error = "La configuration du composant 'ai_llm' est absente des service_settings.",
                context = json_value!({"service_id": "ref:services:handle:svc_ai"})
            ),
        };

        let model_filename = match comp_settings
            .get("rust_model_file")
            .and_then(|v| v.as_str())
        {
            Some(m) => m.to_string(),
            None => raise_error!(
                "ERR_AI_LLM_MODEL_MISSING",
                error = "La clé 'rust_model_file' est introuvable."
            ),
        };

        let tokenizer_filename = match comp_settings
            .get("rust_tokenizer_file")
            .and_then(|v| v.as_str())
        {
            Some(t) => t.to_string(),
            None => raise_error!(
                "ERR_AI_LLM_TOKENIZER_MISSING",
                error = "La clé 'rust_tokenizer_file' est introuvable."
            ),
        };

        // 3. Construction des chemins via les Mount Points (Zéro Dette)
        let config = AppConfig::get();
        let base_path = match config.get_path("PATH_RAISE_DOMAIN") {
            Some(p) => p
                .join(&config.mount_points.system.domain)
                .join(&config.mount_points.system.db)
                .join("ai-assets/models"),
            None => raise_error!(
                "ERR_CONFIG_DOMAIN_PATH_MISSING",
                error = "Le chemin racine 'PATH_RAISE_DOMAIN' est absent de la configuration."
            ),
        };

        let model_path = base_path.join(&model_filename);
        let tokenizer_path = base_path.join(&tokenizer_filename);

        // 4. Vérifications de résilience physique via Match
        if !model_path.exists() {
            raise_error!(
                "ERR_AI_MODEL_FILE_NOT_FOUND",
                error = format!("Modèle GGUF introuvable : {}", model_filename),
                context = json_value!({ "resolved_path": model_path.to_string_lossy() })
            );
        }

        if !tokenizer_path.exists() {
            raise_error!(
                "ERR_AI_TOKENIZER_FILE_NOT_FOUND",
                error = format!("Tokenizer introuvable : {}", tokenizer_filename),
                context = json_value!({ "resolved_path": tokenizer_path.to_string_lossy() })
            );
        }

        // 5. Résolution Hardware (SSOT: AppConfig)
        let device = AppConfig::device().clone();
        user_info!(
            "MSG_AI_ENGINE_LOAD_START",
            json_value!({ "model": model_filename, "device": format!("{:?}", device) })
        );

        // 6. Chargement sécurisé du TextTokenizer
        let tokenizer = match TextTokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_AI_TOKENIZER_LOAD_FAILED",
                error = e.to_string(),
                context = json_value!({ "path": tokenizer_path.to_string_lossy() })
            ),
        };

        // 7. Ouverture et lecture du fichier GGUF
        let mut file = match fs::open_sync(&model_path) {
            Ok(f) => f,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_OPEN_FAILED",
                error = e.to_string(),
                context = json_value!({ "path": model_path.to_string_lossy() })
            ),
        };

        let model_content = match GgufFileFormat::Content::read(&mut file) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_AI_MODEL_READ_CONTENT", error = e.to_string()),
        };

        // 8. Instanciation des poids neuronaux
        let weights =
            match Qwen2QuantizedModel::ModelWeights::from_gguf(model_content, &mut file, &device) {
                Ok(w) => w,
                Err(e) => raise_error!("ERR_AI_QWEN2_WEIGHTS_LOAD", error = e.to_string()),
            };

        Ok(Self {
            model: weights,
            tokenizer,
            device,
            logits_processor: TokenLogitsProcessor::new(299792458, Some(0.7), None),
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
            Err(e) => raise_error!("ERR_TOKENIZER_ENCODE_FAILED", error = e.to_string()),
        };

        let mut tokens = tokens.get_ids().to_vec();
        let mut generated_tokens = Vec::new();
        let mut index_pos = 0;

        // Résolution dynamique des Stop Tokens (ChatML)
        let eos_token_id = match self.tokenizer.token_to_id("<|im_end|>") {
            Some(id) => id,
            None => raise_error!(
                "ERR_AI_FORMAT_INCOMPATIBLE",
                error = "Token <|im_end|> manquant."
            ),
        };

        let stop_token_id = self.tokenizer.token_to_id("<|endoftext|>");

        for _i in 0..max_tokens {
            let context_size = if index_pos == 0 { tokens.len() } else { 1 };
            let start_pos = tokens.len().saturating_sub(context_size);

            let input = match NeuralTensor::new(&tokens[start_pos..], &self.device) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_AI_TENSOR_INPUT_FAILED", error = e.to_string()),
            };

            let input = match input.unsqueeze(0) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_AI_TENSOR_SHAPE_ERROR", error = e.to_string()),
            };

            let logits = match self.model.forward(&input, index_pos) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_AI_FORWARD_PASS_FAILED", error = e.to_string()),
            };

            let logits = match logits.squeeze(0).and_then(|l| l.squeeze(0)) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_AI_TENSOR_REDUCTION_FAILED", error = e.to_string()),
            };

            let logits = match logits.to_dtype(ComputeType::F32) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_AI_DTYPE_CONVERSION_FAILED", error = e.to_string()),
            };

            let next_token = match self.logits_processor.sample(&logits) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_AI_SAMPLING_FAILED", error = e.to_string()),
            };

            if next_token == eos_token_id || Some(next_token) == stop_token_id {
                break;
            }

            tokens.push(next_token);
            generated_tokens.push(next_token);
            index_pos += context_size;
        }

        match self.tokenizer.decode(&generated_tokens, true) {
            Ok(res) => Ok(res),
            Err(e) => raise_error!("ERR_TOKENIZER_DECODE_FAILED", error = e.to_string()),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================
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
        assert_eq!(NativeTensorEngine::format_prompt(sys, user), expected);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_quick_inference() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "llm", json_value!({})).await?;

        let mut engine = NativeTensorEngine::new(&manager).await?;
        let response = engine.generate("Réponds 'OK'.", "Test", 5)?;

        assert!(!response.is_empty());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_missing_model_path() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Injection d'une config pointant vers un fichier fantôme
        inject_mock_component(
            &manager,
            "llm",
            json_value!({
                "rust_model_file": "ghost_model.gguf",
                "rust_tokenizer_file": "ghost_tok.json"
            }),
        )
        .await?;

        let result = NativeTensorEngine::new(&manager).await;

        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_AI_MODEL_FILE_NOT_FOUND");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_AI_MODEL_FILE_NOT_FOUND"),
        }
    }
}
