// FICHIER : src-tauri/src/ai/nlp/embeddings/candle.rs

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
    pub async fn new(
        manager: &crate::json_db::collections::manager::CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        let device = AppConfig::device().clone();
        user_info!(
            "🕯️ [Candle NLP] Moteur initialisé sur : {:?}",
            json_value!(format!("{:?}", device))
        );

        let settings = AppConfig::get_component_settings(manager, "nlp").await?;

        let model_dir = settings
            .get("model_name")
            .and_then(|v| v.as_str())
            .unwrap_or("minilm");
        let config_filename = settings
            .get("rust_config_file")
            .and_then(|v| v.as_str())
            .unwrap_or("config.json");
        let tokenizer_filename = settings
            .get("rust_tokenizer_file")
            .and_then(|v| v.as_str())
            .unwrap_or("tokenizer.json");
        let weights_filename = settings
            .get("rust_safetensors_file")
            .and_then(|v| v.as_str())
            .unwrap_or("model.safetensors");

        let Some(home) = dirs::home_dir() else {
            raise_error!(
                "ERR_OS_HOME_NOT_FOUND",
                error = "Impossible de localiser le répertoire personnel de l'utilisateur (home).",
                context = json_value!({ "method": "dirs::home_dir" })
            );
        };

        let base_path = home.join(format!(
            "raise_domain/_system/ai-assets/embeddings/{}",
            model_dir
        ));
        let config_path = base_path.join(config_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);
        let weights_path = base_path.join(weights_filename);

        if !weights_path.exists() || !config_path.exists() || !tokenizer_path.exists() {
            raise_error!(
                "ERR_AI_EMBEDDING_ASSETS_MISSING",
                error = format!("Fichiers d'embeddings introuvables dans : {:?}", base_path),
                context = json_value!({
                    "base_path": base_path.to_string_lossy(),
                    "missing_files": {
                        "weights": !weights_path.exists(),
                        "config": !config_path.exists(),
                        "tokenizer": !tokenizer_path.exists()
                    }
                })
            );
        }

        let config_str = match fs::read_to_string_sync(&config_path) {
            Ok(content) => content,
            Err(e) => raise_error!(
                "ERR_CONFIG_READ",
                error = e,
                context = json_value!({"path": config_path.to_string_lossy()})
            ),
        };

        let config: Config = match json::deserialize_from_str(&config_str) {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_CONFIG_PARSE",
                error = e,
                context = json_value!({"config_preview": config_str.chars().take(100).collect::<String>()})
            ),
        };

        let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_TOKENIZER_LOAD",
                error = e,
                context = json_value!({"path": tokenizer_path.to_string_lossy()})
            ),
        };

        let vb = unsafe {
            match VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &device) {
                Ok(builder) => builder,
                Err(e) => raise_error!(
                    "ERR_AI_WEIGHTS_LOAD_FAILED",
                    error = e,
                    context = json_value!({"path": weights_path.to_string_lossy()})
                ),
            }
        };

        let model = match BertModel::load(vb, &config) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_INSTANTIATION_FAILED",
                error = e,
                context = json_value!({"model_type": "BERT"})
            ),
        };

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// 🎯 VRAI BATCHING GPU : Tokenise et infère un lot entier en une seule passe tensorielle
    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        // 1. Tokenisation en masse
        let encodings = match self.tokenizer.encode_batch(texts.clone(), true) {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_NLP_BATCH_TOKENIZATION_FAILED",
                error = e,
                context = json_value!({"batch_size": batch_size})
            ),
        };

        // 2. Padding dynamique : Trouver la séquence la plus longue du lot
        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);

        // 3. Préparation des vecteurs plats pour le constructeur Tensor
        let mut batch_ids = Vec::with_capacity(batch_size * max_len);
        let batch_type_ids = vec![0u32; batch_size * max_len]; // Les type_ids sont toujours 0 pour MiniLM

        for enc in &encodings {
            let ids = enc.get_ids();
            batch_ids.extend_from_slice(ids);
            // On pad avec des zéros (le token PAD de BERT) jusqu'à max_len
            batch_ids.resize(batch_ids.len() + (max_len - ids.len()), 0);
        }

        // 4. Création des Tenseurs [Batch_Size, Sequence_Length]
        let token_ids = match Tensor::from_vec(batch_ids, (batch_size, max_len), &self.device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_NLP_BATCH_TENSOR_CREATION", error = e),
        };

        let token_type_ids =
            match Tensor::from_vec(batch_type_ids, (batch_size, max_len), &self.device) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_NLP_BATCH_TYPE_TENSOR", error = e),
            };

        // 5. INFÉRENCE DE MASSE (Le GPU travaille à 100%)
        let embeddings = match self.model.forward(&token_ids, &token_type_ids, None) {
            Ok(emb) => emb,
            Err(e) => raise_error!(
                "ERR_NLP_BATCH_FORWARD",
                error = e,
                context = json_value!({"batch_size": batch_size, "seq_len": max_len})
            ),
        };

        // 6. Pooling (Moyenne sur la dimension des tokens -> dim 1)
        let sum_embeddings = match embeddings.sum(1) {
            Ok(s) => s,
            Err(e) => raise_error!("ERR_NLP_BATCH_SUM", error = e),
        };

        let pooled = match sum_embeddings / (max_len as f64) {
            Ok(p) => p,
            Err(e) => raise_error!("ERR_NLP_BATCH_POOLING", error = e),
        };

        // 7. Normalisation avec Epsilon
        let normalized = normalize_l2(&pooled)?;

        // 8. Conversion [Batch, Hidden] -> Vec<Vec<f32>>
        match normalized.to_vec2::<f32>() {
            Ok(matrix) => Ok(matrix),
            Err(e) => raise_error!("ERR_NLP_BATCH_VEC_CONVERSION", error = e),
        }
    }

    /// Rétrocompatibilité pour une seule requête
    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        let mut batch_res = self.embed_batch(vec![text.to_string()])?;
        batch_res.pop().ok_or_else(|| {
            build_error!(
                "ERR_NLP_EMPTY_BATCH_RESULT",
                error = "embed_batch a retourné un vecteur vide"
            )
        })
    }
}

/// 🎯 CORRECTION MATHÉMATIQUE : Normalisation L2 robuste avec Epsilon
fn normalize_l2(v: &Tensor) -> RaiseResult<Tensor> {
    // 1. Calcul de la somme des carrés (Sum of Squares)
    let sum_sq = match v.sqr() {
        Ok(s) => match s.sum_keepdim(1) {
            Ok(sum) => sum,
            Err(e) => raise_error!("ERR_NLP_NORM_SUM_FAILED", error = e),
        },
        Err(e) => raise_error!("ERR_NLP_NORM_SQR_FAILED", error = e),
    };

    // 2. 🎯 Epsilon de sécurité (1e-8) pour éviter les divisions par zéro
    let epsilon = match Tensor::new(&[1e-8f32], v.device()) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_NLP_NORM_EPSILON", error = e),
    };

    let safe_sum_sq = match sum_sq.broadcast_add(&epsilon) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_NLP_NORM_ADD", error = e),
    };

    let norm = match safe_sum_sq.sqrt() {
        Ok(n) => n,
        Err(e) => raise_error!("ERR_NLP_NORM_SQRT_FAILED", error = e),
    };

    // 3. Division finale
    match v.broadcast_div(&norm) {
        Ok(normalized) => Ok(normalized),
        Err(e) => raise_error!("ERR_NLP_NORM_DIV_FAILED", error = e),
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_mini_lm_loading() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors"})).await;

        let engine = CandleEngine::new(&manager).await;
        assert!(
            engine.is_ok(),
            "Le modèle MiniLM doit se charger correctement"
        );
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_dimensions() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors"})).await;

        let mut engine = CandleEngine::new(&manager).await.expect("Init failed");

        // Test Batching
        let batch = vec![
            "Phrase 1".to_string(),
            "Une phrase beaucoup plus longue pour tester le padding dynamique du batch".to_string(),
        ];
        let vecs = engine.embed_batch(batch).expect("Batch Embed failed");

        assert_eq!(vecs.len(), 2, "Doit retourner 2 vecteurs");
        assert_eq!(vecs[0].len(), 384, "La dimension doit être 384");
        assert_eq!(
            vecs[1].len(),
            384,
            "La dimension doit être 384 (même avec du padding)"
        );
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_normalization() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors"})).await;

        let mut engine = CandleEngine::new(&manager).await.expect("Init failed");
        let vec = engine.embed_query("Mathematiques").expect("Embed failed");

        let sum_sq: f32 = vec.iter().map(|x| x * x).sum();
        let norm = sum_sq.sqrt();

        assert!(
            (norm - 1.0).abs() < 1e-4,
            "Le vecteur doit être normalisé (Norme proche de 1.0), actuel: {}",
            norm
        );
    }
}
