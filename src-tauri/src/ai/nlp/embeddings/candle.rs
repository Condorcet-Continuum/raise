// FICHIER : src-tauri/src/ai/nlp/embeddings/candle.rs

use crate::utils::prelude::*; // 🎯 Façade Unique

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
    /// Initialise le moteur d'embeddings BERT en respectant les points de montage.
    pub async fn new(
        manager: &crate::json_db::collections::manager::CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        let device = AppConfig::device().clone();
        user_info!(
            "MSG_NLP_CANDLE_INIT",
            json_value!({ "device": format!("{:?}", device), "backend": "BERT" })
        );

        // 1. Récupération des paramètres via le point de montage Système
        let settings = AppConfig::get_component_settings(manager, "ai_nlp").await?;

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

        // 2. Résolution dynamique du chemin via AppConfig
        let config_global = AppConfig::get();
        let base_path = match config_global.get_path("PATH_RAISE_DOMAIN") {
            Some(p) => p
                .join(&config_global.mount_points.system.domain) // 🎯 FIX: Usage des mount_points
                .join(&config_global.mount_points.system.db)
                .join("ai-assets/embeddings")
                .join(model_dir),
            None => raise_error!(
                "ERR_CONFIG_DOMAIN_PATH_MISSING",
                error = "Le chemin racine 'PATH_RAISE_DOMAIN' est absent de la configuration."
            ),
        };

        let config_path = base_path.join(config_filename);
        let tokenizer_path = base_path.join(tokenizer_filename);
        let weights_path = base_path.join(weights_filename);

        // 3. Vérification de résilience physique
        if !weights_path.exists() || !config_path.exists() || !tokenizer_path.exists() {
            raise_error!(
                "ERR_AI_EMBEDDING_ASSETS_MISSING",
                error = format!("Modèle BERT introuvable dans : {:?}", base_path),
                context = json_value!({ "path": base_path.to_string_lossy() })
            );
        }

        // 4. Chargement sécurisé de la configuration BERT
        let config_str = match fs::read_to_string_sync(&config_path) {
            Ok(content) => content,
            Err(e) => raise_error!("ERR_NLP_CONFIG_READ", error = e.to_string()),
        };

        let bert_config: Config = match json::deserialize_from_str(&config_str) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_NLP_CONFIG_PARSE", error = e.to_string()),
        };

        // 5. Chargement du Tokenizer
        let tokenizer = match Tokenizer::from_file(&tokenizer_path) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_NLP_TOKENIZER_LOAD", error = e.to_string()),
        };

        // 6. Chargement des poids via Memory Mapping
        let vb = unsafe {
            match VarBuilder::from_mmaped_safetensors(&[&weights_path], DType::F32, &device) {
                Ok(builder) => builder,
                Err(e) => raise_error!("ERR_NLP_WEIGHTS_LOAD", error = e.to_string()),
            }
        };

        let model = match BertModel::load(vb, &bert_config) {
            Ok(m) => m,
            Err(e) => raise_error!("ERR_NLP_MODEL_INSTANTIATION", error = e.to_string()),
        };

        Ok(Self {
            model,
            tokenizer,
            device,
        })
    }

    /// Vectorise un lot de textes (Batch Inference)
    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        let encodings = match self.tokenizer.encode_batch(texts, true) {
            Ok(e) => e,
            Err(e) => raise_error!("ERR_NLP_BATCH_TOKENIZATION", error = e.to_string()),
        };

        let max_len = encodings
            .iter()
            .map(|e| e.get_ids().len())
            .max()
            .unwrap_or(0);
        let mut batch_ids = Vec::with_capacity(batch_size * max_len);
        let batch_type_ids = vec![0u32; batch_size * max_len];

        for enc in &encodings {
            let ids = enc.get_ids();
            batch_ids.extend_from_slice(ids);
            batch_ids.resize(batch_ids.len() + (max_len - ids.len()), 0);
        }

        let token_ids = match Tensor::from_vec(batch_ids, (batch_size, max_len), &self.device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_NLP_TENSOR_IDS", error = e.to_string()),
        };

        let token_type_ids =
            match Tensor::from_vec(batch_type_ids, (batch_size, max_len), &self.device) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_NLP_TENSOR_TYPES", error = e.to_string()),
            };

        let embeddings = match self.model.forward(&token_ids, &token_type_ids, None) {
            Ok(emb) => emb,
            Err(e) => raise_error!("ERR_NLP_FORWARD_PASS", error = e.to_string()),
        };

        // Pooling sémantique (Moyenne)
        // 🎯 FIX: Suppression des parenthèses inutiles signalées par le compilateur
        let pooled = match embeddings.sum(1)? / (max_len as f64) {
            Ok(p) => p,
            Err(e) => raise_error!("ERR_NLP_POOLING", error = e.to_string()),
        };

        let normalized = normalize_l2(&pooled)?;

        match normalized.to_vec2::<f32>() {
            Ok(matrix) => Ok(matrix),
            Err(e) => raise_error!("ERR_NLP_VEC_CONVERSION", error = e.to_string()),
        }
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        let mut batch_res = self.embed_batch(vec![text.to_string()])?;
        batch_res
            .pop()
            .ok_or_else(|| build_error!("ERR_NLP_EMPTY_RESULT", error = "Inférence nulle"))
    }
}

fn normalize_l2(v: &Tensor) -> RaiseResult<Tensor> {
    let sum_sq = v.sqr()?.sum_keepdim(1)?;
    let epsilon = Tensor::new(&[1e-8f32], v.device())?;
    let norm = sum_sq.broadcast_add(&epsilon)?.sqrt()?;

    match v.broadcast_div(&norm) {
        Ok(normalized) => Ok(normalized),
        Err(e) => raise_error!("ERR_NLP_NORMALIZATION_FAILED", error = e.to_string()),
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    async fn provide_assets_to_sandbox(model_name: &str) {
        let config = AppConfig::get();
        let domain_path = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_default_domain"));

        if let Some(home) = dirs::home_dir() {
            let real_path = home.join(format!(
                "raise_domain/_system/ai-assets/embeddings/{}",
                model_name
            ));
            // 🎯 FIX: Alignement des chemins de test sur les nouveaux points de montage
            let target_path = domain_path
                .join(&config.mount_points.system.domain)
                .join(&config.mount_points.system.db)
                .join(format!("ai-assets/embeddings/{}", model_name));

            if fs::exists_async(&real_path).await {
                let _ = fs::ensure_dir_async(&target_path).await;
                let _ = fs::copy_async(
                    real_path.join("config.json"),
                    target_path.join("config.json"),
                )
                .await;
                let _ = fs::copy_async(
                    real_path.join("tokenizer.json"),
                    target_path.join("tokenizer.json"),
                )
                .await;
                let _ = fs::copy_async(
                    real_path.join("model.safetensors"),
                    target_path.join("model.safetensors"),
                )
                .await;
            }
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_mini_lm_loading() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        provide_assets_to_sandbox("minilm").await;

        // 🎯 FIX: Utilisation des mount_points pour le manager de test
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm"})).await;

        let engine = CandleEngine::new(&manager).await?;
        assert!(engine.tokenizer.get_vocab_size(true) > 0);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_candle_dimensions_and_norm() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        provide_assets_to_sandbox("minilm").await;

        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm"})).await;

        let mut engine = CandleEngine::new(&manager).await?;
        let vec = engine.embed_query("Test NLP")?;

        assert_eq!(vec.len(), 384);
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-4);
        Ok(())
    }

    #[async_test]
    async fn test_resilience_missing_nlp_assets() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "nlp", json_value!({"model_name": "ghost_model"})).await;

        let result = CandleEngine::new(&manager).await;
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_AI_EMBEDDING_ASSETS_MISSING");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_AI_EMBEDDING_ASSETS_MISSING"),
        }
    }
}
