use crate::utils::prelude::*;

use candle_core::{DType, Device, Tensor};
use candle_nn::VarBuilder;
use candle_transformers::models::bert::{BertModel, Config};
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

        println!("🕯️ [Candle NLP] Moteur initialisé sur : {:?}", device);

        // 2. RÉCUPÉRATION DE LA CONFIGURATION DYNAMIQUE SSOT
        let config_app = AppConfig::get();
        let engine_cfg = config_app
            .ai_engines
            .get("primary_embedding")
            .ok_or_else(|| {
                AppError::Ai(
                    "Moteur 'primary_embedding' introuvable dans la configuration".to_string(),
                )
            })?;

        // Extraction des noms de fichiers (avec fallbacks)
        let model_dir = &engine_cfg.model_name; // Ex: "minilm"
        let config_filename = engine_cfg
            .rust_config_file
            .as_deref()
            .unwrap_or("config.json");
        let tokenizer_filename = engine_cfg
            .rust_tokenizer_file
            .as_deref()
            .unwrap_or("tokenizer.json");
        let weights_filename = engine_cfg
            .rust_safetensors_file
            .as_deref()
            .unwrap_or("model.safetensors");

        // 3. CONSTRUCTION DES CHEMINS LOCAUX ABSOLUS
        let home = dirs::home_dir().ok_or_else(|| {
            AppError::Ai("Impossible de trouver le dossier utilisateur (home)".to_string())
        })?;

        // On cible dynamiquement le dossier du modèle (ex: embeddings/minilm)
        let base_path = home.join(format!(
            "raise_domain/_system/ai-assets/embeddings/{}",
            model_dir
        ));

        let config_path = base_path.join(config_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);
        let weights_path = base_path.join(weights_filename);

        // 3. Vérification de sécurité stricte
        if !weights_path.exists() || !config_path.exists() || !tokenizer_path.exists() {
            return Err(AppError::Ai(format!(
                "Fichiers d'embeddings introuvables en local. Vérifiez le dossier : {:?}",
                base_path
            )));
        }

        // 4. Chargement de la configuration
        let config_str = std::fs::read_to_string(&config_path)
            .map_err(|e| AppError::from(format!("Erreur lecture config: {}", e)))?;

        // Utilisation de serde_json (ou data::parse) pour lire la config Bert
        let config: Config = serde_json::from_str(&config_str)
            .map_err(|e| AppError::from(format!("Erreur parsing config: {}", e)))?;

        // 5. Chargement du Tokenizer
        let tokenizer = Tokenizer::from_file(&tokenizer_path)
            .map_err(|e| AppError::from(format!("Erreur tokenizer: {}", e)))?;

        // 6. Chargement des poids (Safetensors)
        let vb = unsafe {
            VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &device)
                .map_err(|e| AppError::from(e.to_string()))?
        };

        // 7. Initialisation du modèle Bert
        let model = BertModel::load(vb, &config).map_err(|e| AppError::from(e.to_string()))?;

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

        let token_ids = Tensor::new(tokens.get_ids(), &self.device)
            .map_err(|e| AppError::from(e.to_string()))?
            .unsqueeze(0)
            .map_err(|e| AppError::from(e.to_string()))?;

        let token_type_ids = token_ids
            .zeros_like()
            .map_err(|e| AppError::from(e.to_string()))?;

        let embeddings = self
            .model
            .forward(&token_ids, &token_type_ids, None)
            .map_err(|e| AppError::from(e.to_string()))?;

        let (_n_sentence, n_tokens, _hidden_size) = embeddings
            .dims3()
            .map_err(|e| AppError::from(e.to_string()))?;

        let sum_embeddings = embeddings
            .sum(1)
            .map_err(|e| AppError::from(e.to_string()))?;
        let embeddings =
            (sum_embeddings / (n_tokens as f64)).map_err(|e| AppError::from(e.to_string()))?;

        let embeddings = normalize_l2(&embeddings)?;

        let vec = embeddings
            .squeeze(0)
            .map_err(|e| AppError::from(e.to_string()))?
            .to_vec1::<f32>()
            .map_err(|e| AppError::from(e.to_string()))?;
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
    let sum_sq = v
        .sqr()
        .map_err(|e| AppError::from(e.to_string()))?
        .sum_keepdim(1)
        .map_err(|e| AppError::from(e.to_string()))?;

    let norm = sum_sq.sqrt().map_err(|e| AppError::from(e.to_string()))?;

    v.broadcast_div(&norm)
        .map_err(|e| AppError::from(e.to_string()))
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
