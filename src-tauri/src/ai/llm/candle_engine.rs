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
        // 1. Récupération de la configuration globale dynamique
        let settings = AppConfig::get_component_settings(manager, "llm").await?;

        // 2. Lecture simple des paramètres
        let model_filename = settings
            .get("rust_model_file")
            .and_then(|v| v.as_str())
            .unwrap_or("qwen2.5-1.5b-instruct-q4_k_m.gguf");
        let tokenizer_filename = settings
            .get("rust_tokenizer_file")
            .and_then(|v| v.as_str())
            .unwrap_or("tokenizer.json");

        // 3. Construction des chemins LOCAUX absolus (100% Hors-ligne)
        let Some(home) = dirs::home_dir() else {
            raise_error!(
                "ERR_OS_HOME_NOT_FOUND",
                error = "Impossible de localiser le répertoire personnel de l'utilisateur (HOME).",
                context = json_value!({ "method": "dirs::home_dir" })
            );
        };

        let base_path = home.join("raise_domain/_system/ai-assets/models");
        let model_path = base_path.join(model_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);

        // 4. Vérification de sécurité stricte
        if !model_path.exists() {
            raise_error!(
                "ERR_AI_MODEL_FILE_NOT_FOUND",
                error = format!("Modèle GGUF introuvable en local : {:?}", model_path),
                context = json_value!({ "path": model_path.to_string_lossy() })
            );
        }
        if !tokenizer_path.exists() {
            raise_error!(
                "ERR_AI_TOKENIZER_FILE_NOT_FOUND",
                error = format!("Tokenizer introuvable en local : {:?}", tokenizer_path),
                context = json_value!({ "path": tokenizer_path.to_string_lossy() })
            );
        }

        // 5. Initialisation matérielle (CPU par défaut)
        // 5. DÉTECTION MATÉRIELLE DYNAMIQUE (Priorité absolue : CUDA)
        let device = candle_core::Device::new_cuda(0).unwrap_or(candle_core::Device::Cpu); // Fallback CPU si erreur

        println!("🚀 [Candle LLM] Moteur Qwen chargé sur : {:?}", device);

        // 6. Chargement du Tokenizer depuis le fichier local
        let tokenizer = match tokenizers::Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => {
                // La macro gère déjà le retour divergent
                raise_error!(
                    "ERR_AI_TOKENIZER_LOAD_FAILED",
                    error = e,
                    context = json_value!({
                        "action": "initialize_tokenizer",
                        "path": tokenizer_path.to_string_lossy(),
                        "hint": "Le fichier 'tokenizer.json' est introuvable ou malformé. Vérifiez que le modèle a été correctement téléchargé dans le dossier 'assets/models'."
                    })
                )
            }
        };

        // 7. Chargement du modèle GGUF depuis le fichier local
        let mut file = match std::fs::File::open(&model_path) {
            Ok(f) => f,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_OPEN",
                error = e,
                context = json_value!({ "path": model_path.to_string_lossy() })
            ),
        };

        let model = match gguf_file::Content::read(&mut file) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_READ_CONTENT",
                error = e,
                context = json_value!({
                    "action": "READ_GGUF_CONTENT",
                    "path": model_path.to_string_lossy()
                })
            ),
        };
        // 8. Initialisation de l'architecture Qwen2
        let weights = match candle_transformers::models::quantized_qwen2::ModelWeights::from_gguf(
            model, &mut file, &device,
        ) {
            Ok(w) => w,
            Err(e) => raise_error!(
                "ERR_AI_QWEN2_WEIGHTS_LOAD",
                error = e,
                context = json_value!({
                    "model_family": "Qwen2",
                    "path": model_path.to_string_lossy()
                })
            ),
        }; // 9. Initialisation du processeur de texte (Température 0.7 par défaut)
        let logits_processor = LogitsProcessor::new(299792458, Some(0.7), None);

        // Retour de l'instance
        Ok(Self {
            model: weights,
            tokenizer,
            device,
            logits_processor, // 🎯 On n'oublie pas de le retourner !
        })
    }

    fn format_prompt(system_prompt: &str, user_prompt: &str) -> String {
        // Format ChatML utilisé par Qwen 2.5
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
                context = json_value!({
                    "action": "encode_prompt",
                    // Un petit plus pour l'IA : on capture un aperçu du prompt qui a fait planter !
                    "prompt_preview": formatted_prompt.chars().take(50).collect::<String>()
                })
            ),
        };

        let mut tokens = tokens.get_ids().to_vec();
        let mut generated_tokens = Vec::new();

        let mut index_pos = 0;
        // Token standard de fin de message pour ChatML / Qwen
        let eos_token_id = self.tokenizer.token_to_id("<|im_end|>").unwrap_or(151645);
        // Token global de fin de texte pour Qwen
        let stop_token_id = self
            .tokenizer
            .token_to_id("<|endoftext|>")
            .unwrap_or(151643);

        for _i in 0..max_tokens {
            let context_size = if index_pos == 0 { tokens.len() } else { 1 };
            let start_pos = tokens.len().saturating_sub(context_size);

            // 1. Tenseur d'entrée
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

            // 2. Forward Pass
            let logits = match self.model.forward(&input, index_pos) {
                Ok(l) => l,
                Err(e) => raise_error!(
                    "ERR_AI_FORWARD_PASS_FAILED",
                    error = e,
                    context = json_value!({ "index_pos": index_pos })
                ),
            };

            // 3. Post-traitement (Extraction manuelle pour éviter le mismatch)
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

            // 4. Sampling
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
                context = json_value!({
                    "action": "decode_tokens",
                    // Info ultra-utile pour l'IA/Debug : combien de tokens ont fait planter le décodeur ?
                    "token_count": generated_tokens.len()
                })
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
        inject_mock_component(
            &manager,
            "llm",
             json_value!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

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
