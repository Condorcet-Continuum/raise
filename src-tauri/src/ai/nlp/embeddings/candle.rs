use anyhow::Result;
use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
use hf_hub::{api::sync::Api, Repo, RepoType};
use tokenizers::Tokenizer;

pub struct CandleEngine {
    model: BertModel,
    tokenizer: Tokenizer,
    device: Device,
}

impl CandleEngine {
    pub fn new() -> Result<Self> {
        // 1. DÉTECTION DYNAMIQUE DU MATÉRIEL (GPU > CPU)
        let device = Device::new_metal(0) // Apple Silicon (M1/M2/M3)
            .or_else(|_| Device::new_cuda(0)) // Nvidia CUDA
            .unwrap_or(Device::Cpu); // Fallback CPU standard

        println!("🕯️ [Candle] Moteur initialisé sur : {:?}", device);

        let api = Api::new()?;
        let repo = api.repo(Repo::new(
            "sentence-transformers/all-MiniLM-L6-v2".to_string(),
            RepoType::Model,
        ));

        let config_path = repo.get("config.json")?;
        let tokenizer_path = repo.get("tokenizer.json")?;
        let weights_path = repo.get("model.safetensors")?;

        let config_str = std::fs::read_to_string(config_path)?;
        let config: Config = serde_json::from_str(&config_str)?;
        let tokenizer = Tokenizer::from_file(tokenizer_path).map_err(anyhow::Error::msg)?;

        let vb =
            unsafe { VarBuilder::from_mmaped_safetensors(&[weights_path], DType::F32, &device)? };
        let model = BertModel::load(vb, &config)?;

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    fn forward_one(&self, text: &str) -> Result<Vec<f32>> {
        let tokens = self
            .tokenizer
            .encode(text, true)
            .map_err(anyhow::Error::msg)?;

        // On s'assure que les tenseurs sont créés sur le bon Device
        let token_ids = Tensor::new(tokens.get_ids(), &self.device)?.unsqueeze(0)?;
        let token_type_ids = token_ids.zeros_like()?;

        let embeddings = self.model.forward(&token_ids, &token_type_ids, None)?;

        let (_n_sentence, n_tokens, _hidden_size) = embeddings.dims3()?;
        let embeddings = (embeddings.sum(1)? / (n_tokens as f64))?;

        let embeddings = normalize_l2(&embeddings)?;

        let vec = embeddings.squeeze(0)?.to_vec1::<f32>()?;
        Ok(vec)
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        // Note: Pour une optimisation future, on pourrait tokeniser tout le batch
        // et faire un seul appel forward(), mais cela demande de gérer le Padding manuellement.
        // Avec le GPU activé, cette boucle sera déjà très rapide pour des petits lots.
        let mut results = Vec::new();
        for text in texts {
            results.push(self.forward_one(&text)?);
        }
        Ok(results)
    }

    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        self.forward_one(text)
    }
}

fn normalize_l2(v: &Tensor) -> Result<Tensor> {
    let sum_sq = v.sqr()?.sum_keepdim(1)?;
    let norm = sum_sq.sqrt()?;
    Ok(v.broadcast_div(&norm)?)
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_candle_mini_lm_loading() {
        let engine = CandleEngine::new();
        assert!(
            engine.is_ok(),
            "Le modèle MiniLM doit se charger correctement via HF Hub"
        );
    }

    #[test]
    fn test_candle_dimensions() {
        let mut engine = CandleEngine::new().expect("Init failed");
        let vec = engine.embed_query("Test dimensions").expect("Embed failed");

        // all-MiniLM-L6-v2 fait 384 dimensions
        assert_eq!(vec.len(), 384);
    }

    #[test]
    fn test_candle_normalization() {
        // Vérifie que le vecteur est normalisé (L2 norm ≈ 1.0)
        // C'est CRUCIAL pour que la Cosine Similarity fonctionne dans Qdrant
        let mut engine = CandleEngine::new().expect("Init failed");
        let vec = engine.embed_query("Mathematiques").expect("Embed failed");

        let sum_sq: f32 = vec.iter().map(|x| x * x).sum();
        let norm = sum_sq.sqrt();

        // On tolère une petite erreur de virgule flottante
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "Le vecteur doit être normalisé (Norme proche de 1.0), actuel: {}",
            norm
        );
    }
}
