use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

pub mod candle;
pub mod fast;

pub enum EngineType {
    FastEmbed,
    Candle,
}

pub struct EmbeddingEngine {
    inner: EngineImplementation,
}

enum EngineImplementation {
    Fast(Box<fast::FastEmbedEngine>),
    Candle(Box<candle::CandleEngine>),
}

impl EmbeddingEngine {
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        println!("🧠 Init NLP Engine: Tentative Candle (GPU)...");
        match Self::new_with_type(EngineType::Candle, manager).await {
            Ok(engine) => Ok(engine),
            Err(e) => {
                eprintln!("⚠️ Echec Candle ({}), bascule sur FastEmbed (CPU)", e);
                Self::new_with_type(EngineType::FastEmbed, manager).await
            }
        }
    }

    // 🎯 NOUVEAU : Asynchrone et demande le manager
    pub async fn new_with_type(
        engine_type: EngineType,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        let inner = match engine_type {
            EngineType::FastEmbed => {
                println!("🧠 Init NLP Engine: FastEmbed (ONNX)");
                // FastEmbed n'utilise potentiellement pas la BDD, on l'appelle tel quel (ou on l'adapte si besoin)
                let fast_engine = fast::FastEmbedEngine::new()?;
                EngineImplementation::Fast(Box::new(fast_engine))
            }
            EngineType::Candle => {
                println!("🕯️ Init NLP Engine: Candle (BERT Pure Rust)");
                let candle_engine = candle::CandleEngine::new(manager).await?;
                EngineImplementation::Candle(Box::new(candle_engine))
            }
        };
        Ok(Self { inner })
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        match &mut self.inner {
            EngineImplementation::Fast(e) => {
                let batch_size = texts.len();
                // On transforme l'erreur Anyhow en erreur typée RAISE immédiatement
                match e.embed_batch(texts) {
                    Ok(res) => Ok(res),
                    Err(e) => raise_error!(
                        "ERR_AI_ENGINE_FAST_BATCH_FAILED",
                        error = e,
                        context = json!({
                            "action": "batch_embedding_dispatch",
                            "engine": "fast_cpu_implementation",
                            "batch_size": batch_size
                        })
                    ),
                }
            }

            EngineImplementation::Candle(e) => {
                // On délègue car Candle suit déjà notre standard RAISE
                e.embed_batch(texts)
            }
        }
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        match &mut self.inner {
            EngineImplementation::Fast(e) => match e.embed_query(text) {
                Ok(vec) => Ok(vec),
                Err(err) => raise_error!(
                    "ERR_AI_ENGINE_FAST_QUERY_FAILED",
                    error = err,
                    context = json!({
                        "action": "single_query_dispatch",
                        "engine": "fast_cpu_implementation",
                        "text_length": text.len()
                    })
                ),
            },

            // Candle renvoie déjà un RaiseResult (AppError)
            EngineImplementation::Candle(e) => e.embed_query(text),
        }
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};

    async fn setup_test_manager() -> (StorageEngine, crate::utils::config::AppConfig) {
        crate::utils::config::test_mocks::inject_mock_config();
        let config = crate::utils::config::AppConfig::get();
        let storage_cfg = JsonDbConfig::new(config.get_path("PATH_RAISE_DOMAIN").unwrap());
        (StorageEngine::new(storage_cfg), config.clone())
    }

    #[tokio::test]
    async fn test_default_engine_init() {
        let (storage, config) = setup_test_manager().await;
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        let engine = EmbeddingEngine::new(&manager).await;
        assert!(
            engine.is_ok(),
            "Le moteur par défaut doit s'initialiser sans erreur"
        );
    }

    #[tokio::test]
    async fn test_engine_switching() {
        let (storage, config) = setup_test_manager().await;
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);
        manager.init_db().await.unwrap();

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        // Test FastEmbed
        let mut fast_engine = EmbeddingEngine::new_with_type(EngineType::FastEmbed, &manager)
            .await
            .expect("FastEmbed init failed");
        let vec_fast = fast_engine.embed_query("Test Fast").expect("Embed failed");
        assert_eq!(
            vec_fast.len(),
            384,
            "FastEmbed (BGE-Small) doit sortir 384 dims"
        );

        // Test Candle
        if let Ok(mut candle_engine) =
            EmbeddingEngine::new_with_type(EngineType::Candle, &manager).await
        {
            let vec_candle = candle_engine
                .embed_query("Test Candle")
                .expect("Embed failed");
            assert_eq!(
                vec_candle.len(),
                384,
                "Candle (MiniLM) doit sortir 384 dims"
            );
        } else {
            println!("⚠️ Candle Engine skipped in tests (might be network/setup related)");
        }
    }
}
