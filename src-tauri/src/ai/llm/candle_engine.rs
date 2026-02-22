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
    pub fn new() -> Result<Self> {
        // 1. Récupération de la configuration globale dynamique
        let config = crate::utils::config::AppConfig::get();
        let engine_cfg = config.ai_engines.get("primary_local").ok_or_else(|| {
            crate::utils::error::AppError::Ai(
                "Moteur 'primary_local' introuvable dans la configuration".to_string(),
            )
        })?;

        // 2. Récupération des noms de fichiers (avec fallback par sécurité)
        let model_filename = engine_cfg
            .rust_model_file
            .as_deref()
            .unwrap_or("qwen2.5-1.5b-instruct-q4_k_m.gguf");
        let tokenizer_filename = engine_cfg
            .rust_tokenizer_file
            .as_deref()
            .unwrap_or("tokenizer.json");

        // 3. Construction des chemins LOCAUX absolus (100% Hors-ligne)
        let home = dirs::home_dir().ok_or_else(|| {
            crate::utils::error::AppError::Ai(
                "Impossible de trouver le dossier utilisateur (home)".to_string(),
            )
        })?;

        let base_path = home.join("raise_domain/_system/ai-assets/models");
        let model_path = base_path.join(model_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);

        // 4. Vérification de sécurité stricte
        if !model_path.exists() {
            return Err(crate::utils::error::AppError::Ai(format!(
                "Modèle GGUF introuvable en local : {:?}",
                model_path
            )));
        }
        if !tokenizer_path.exists() {
            return Err(crate::utils::error::AppError::Ai(format!(
                "Tokenizer introuvable en local : {:?}",
                tokenizer_path
            )));
        }

        // 5. Initialisation matérielle (CPU par défaut)
        // 5. DÉTECTION MATÉRIELLE DYNAMIQUE (Priorité absolue : CUDA)
        let device = candle_core::Device::new_cuda(0).unwrap_or(candle_core::Device::Cpu); // Fallback CPU si erreur

        println!("🚀 [Candle LLM] Moteur Qwen chargé sur : {:?}", device);

        // 6. Chargement du Tokenizer depuis le fichier local
        let tokenizer = tokenizers::Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| crate::utils::error::AppError::Ai(format!("Erreur Tokenizer: {}", e)))?;

        // 7. Chargement du modèle GGUF depuis le fichier local
        let mut file = std::fs::File::open(&model_path).map_err(|e| {
            crate::utils::error::AppError::Ai(format!("Erreur ouverture GGUF: {}", e))
        })?;

        let model = gguf_file::Content::read(&mut file).map_err(|e| {
            crate::utils::error::AppError::Ai(format!("Erreur lecture GGUF: {}", e))
        })?;

        // 8. Initialisation de l'architecture Qwen2
        let weights = candle_transformers::models::quantized_qwen2::ModelWeights::from_gguf(
            model, &mut file, &device,
        )
        .map_err(|e| crate::utils::error::AppError::Ai(format!("Erreur poids Qwen2: {}", e)))?;

        // 9. Initialisation du processeur de texte (Température 0.7 par défaut)
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
    ) -> Result<String> {
        let formatted_prompt = Self::format_prompt(system_prompt, user_prompt);

        let tokens = self
            .tokenizer
            .encode(formatted_prompt, true)
            .map_err(|e| AppError::from(format!("Erreur encodage Tokenizer: {}", e)))?;
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

            // ✅ Conversion explicite des erreurs CandleCore vers AppError
            let input = Tensor::new(&tokens[start_pos..], &self.device)
                .map_err(|e| AppError::from(e.to_string()))?
                .unsqueeze(0)
                .map_err(|e| AppError::from(e.to_string()))?;

            let logits = self
                .model
                .forward(&input, index_pos)
                .map_err(|e| AppError::from(e.to_string()))?;
            let logits = logits
                .squeeze(0)
                .map_err(|e| AppError::from(e.to_string()))?
                .squeeze(0)
                .map_err(|e| AppError::from(e.to_string()))?
                .to_dtype(candle_core::DType::F32)
                .map_err(|e| AppError::from(e.to_string()))?;

            let next_token = self
                .logits_processor
                .sample(&logits)
                .map_err(|e| AppError::from(e.to_string()))?;

            if next_token == eos_token_id || next_token == stop_token_id {
                break;
            }

            tokens.push(next_token);
            generated_tokens.push(next_token);
            index_pos += context_size;
        }

        let result = self
            .tokenizer
            .decode(&generated_tokens, true)
            .map_err(|e| AppError::from(format!("Erreur décodage Tokenizer: {}", e)))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::test_mocks::inject_mock_config;

    #[test]
    fn test_qwen_chatml_format() {
        let sys = "Sys";
        let user = "User";
        let expected = "<|im_start|>system\nSys<|im_end|>\n<|im_start|>user\nUser<|im_end|>\n<|im_start|>assistant\n";
        assert_eq!(CandleLlmEngine::format_prompt(sys, user), expected);
    }

    #[test]
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_quick_inference() {
        inject_mock_config();
        println!("Init Engine (Qwen 2.5 1.5B - Rapide)...");

        // Ce test va utiliser le mock pour trouver la config primary_local
        let mut engine = CandleLlmEngine::new().expect("Echec Init Engine");

        println!("🤖 Generating...");
        let response = engine
            .generate("Réponds 'OK'.", "Tu m'entends ?", 10)
            .expect("Echec Generation");

        println!("📝 Réponse: {}", response);
        assert!(!response.is_empty());
    }
}
