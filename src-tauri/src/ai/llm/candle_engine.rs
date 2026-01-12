use anyhow::{Error as E, Result};
use candle_core::quantized::gguf_file;
use candle_core::{Device, Tensor};
use candle_transformers::generation::LogitsProcessor;
use candle_transformers::models::quantized_llama as model;
use dotenvy::dotenv;
use hf_hub::{api::sync::Api, Repo, RepoType};
use std::env;
use tokenizers::Tokenizer;
use tracing::{info, warn};

// --- CONFIGURATION ULTRA-LÉGÈRE (Llama 3.2 1B) ---
// Poids : ~700 Mo (Téléchargement très rapide)
const DEFAULT_REPO_ID: &str = "bartowski/Llama-3.2-1B-Instruct-GGUF";
const DEFAULT_MODEL_FILE: &str = "Llama-3.2-1B-Instruct-Q4_K_M.gguf";

// Tokenizer : On utilise "unsloth" car c'est le seul repo public 100% fiable
const DEFAULT_TOKENIZER_REPO: &str = "unsloth/Llama-3.2-1B-Instruct";

pub struct CandleLlmEngine {
    model: model::ModelWeights,
    tokenizer: Tokenizer,
    device: Device,
    logits_processor: LogitsProcessor,
}

impl CandleLlmEngine {
    pub fn new() -> Result<Self> {
        let _ = dotenv();

        // 1. Configuration (Priorité : Variables d'env, sinon Défaut Rapide)
        // On force le mode "Rapide" si on détecte une config "Lourde" (Mistral/Qwen) dans le .env qui traîne
        let env_repo = env::var("LLM_RUST_REPO_ID").unwrap_or_default();

        let (repo_id, model_file) = if env_repo.to_lowercase().contains("mistral")
            || env_repo.to_lowercase().contains("qwen")
        {
            warn!("🛑 Annulation du modèle lourd détecté dans .env ({}). On force Llama 3.2 1B pour un démarrage immédiat.", env_repo);
            (DEFAULT_REPO_ID.to_string(), DEFAULT_MODEL_FILE.to_string())
        } else {
            (
                env::var("LLM_RUST_REPO_ID").unwrap_or_else(|_| DEFAULT_REPO_ID.to_string()),
                env::var("LLM_RUST_MODEL_FILE").unwrap_or_else(|_| DEFAULT_MODEL_FILE.to_string()),
            )
        };

        // Sécurité Tokenizer : On force Unsloth pour éviter les erreurs 401/404
        let tokenizer_repo = DEFAULT_TOKENIZER_REPO.to_string();

        // 2. Détection Matériel
        let device = Device::new_metal(0)
            .or_else(|_| Device::new_cuda(0))
            .unwrap_or(Device::Cpu);

        info!(
            "🚀 Init Candle Engine (Rapide) sur : {:?} avec {}",
            device, model_file
        );

        // 3. Téléchargement
        let api = Api::new()?;

        let model_repo = api.repo(Repo::new(repo_id, RepoType::Model));
        let model_path = model_repo
            .get(&model_file)
            .map_err(|e| E::msg(format!("Erreur téléchargement modèle: {}", e)))?;
        info!("📦 Modèle chargé : {:?}", model_path);

        let tokenizer_api = api.repo(Repo::new(tokenizer_repo, RepoType::Model));
        let tokenizer_path = tokenizer_api
            .get("tokenizer.json")
            .map_err(|e| E::msg(format!("Erreur téléchargement tokenizer: {}", e)))?;

        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(E::msg)?;

        // 4. Chargement en Mémoire
        let mut file = std::fs::File::open(&model_path)?;
        let content = gguf_file::Content::read(&mut file)
            .map_err(|e| E::msg(format!("Erreur lecture structure GGUF: {}", e)))?;

        let model = model::ModelWeights::from_gguf(content, &mut file, &device)
            .map_err(|e| E::msg(format!("Erreur chargement poids GGUF: {}", e)))?;

        let logits_processor = LogitsProcessor::new(299792458, Some(0.7), Some(0.9));

        Ok(Self {
            model,
            tokenizer,
            device,
            logits_processor,
        })
    }

    fn format_prompt(system_prompt: &str, user_prompt: &str) -> String {
        format!(
            "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>user<|end_header_id|>\n\n{}<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n",
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
            .map_err(E::msg)?;
        let mut tokens = tokens.get_ids().to_vec();
        let mut generated_tokens = Vec::new();

        let mut index_pos = 0;
        let eos_token_id = self.tokenizer.token_to_id("<|eot_id|>").unwrap_or(128009);
        let stop_token_id = self
            .tokenizer
            .token_to_id("<|end_of_text|>")
            .unwrap_or(128001);

        for _i in 0..max_tokens {
            let context_size = if index_pos == 0 { tokens.len() } else { 1 };
            let start_pos = tokens.len().saturating_sub(context_size);

            let input = Tensor::new(&tokens[start_pos..], &self.device)?.unsqueeze(0)?;
            let logits = self.model.forward(&input, index_pos)?;
            let logits = logits
                .squeeze(0)?
                .squeeze(0)?
                .to_dtype(candle_core::DType::F32)?;

            let next_token = self.logits_processor.sample(&logits)?;

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
            .map_err(E::msg)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_llama3_format() {
        let sys = "Sys";
        let user = "User";
        let expected = "<|begin_of_text|><|start_header_id|>system<|end_header_id|>\n\nSys<|eot_id|><|start_header_id|>user<|end_header_id|>\n\nUser<|eot_id|><|start_header_id|>assistant<|end_header_id|>\n\n";
        assert_eq!(CandleLlmEngine::format_prompt(sys, user), expected);
    }

    #[test]
    #[ignore]
    fn test_quick_inference() {
        let _ = dotenv();
        println!("⏳ Init Engine (Llama 3.2 1B - Rapide)...");

        // Ce test va ignorer le .env s'il contient "Mistral" et charger le modèle léger
        let mut engine = CandleLlmEngine::new().expect("Echec Init Engine");

        println!("🤖 Generating...");
        let response = engine
            .generate("Réponds 'OK'.", "Tu m'entends ?", 10)
            .expect("Echec Generation");

        println!("📝 Réponse: {}", response);
        assert!(!response.is_empty());
    }
}
