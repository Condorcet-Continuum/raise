// FICHIER : src-tauri/src/ai/nlp/embeddings/mod.rs

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
        user_info!("🧠 [NLP] Initialisation du moteur: Tentative Candle (GPU)...");

        match Self::new_with_type(EngineType::Candle, manager).await {
            Ok(engine) => Ok(engine),
            Err(e) => {
                // 🎯 OPTIMISATION : On utilise le système de logs de Raise au lieu de eprintln!
                user_warn!(
                    "WRN_NLP_CANDLE_FALLBACK",
                    json_value!({
                        "error": e.to_string(),
                        "action": "fallback_to_fastembed",
                        "hint": "Candle a échoué (souvent dû à l'absence des poids ou de CUDA). Bascule automatique sur FastEmbed (CPU)."
                    })
                );
                Self::new_with_type(EngineType::FastEmbed, manager).await
            }
        }
    }

    pub async fn new_with_type(
        engine_type: EngineType,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        let inner = match engine_type {
            EngineType::FastEmbed => {
                user_info!("🧠 [NLP] Moteur activé : FastEmbed (CPU/ONNX)");
                let fast_engine = fast::FastEmbedEngine::new()?;
                EngineImplementation::Fast(Box::new(fast_engine))
            }
            EngineType::Candle => {
                user_info!("🕯️ [NLP] Moteur activé : Candle (BERT Pure Rust)");
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
                match e.embed_batch(texts) {
                    Ok(res) => Ok(res),
                    Err(err) => raise_error!(
                        "ERR_AI_ENGINE_FAST_BATCH_FAILED",
                        error = err,
                        context = json_value!({
                            "action": "batch_embedding_dispatch",
                            "engine": "fast_cpu_implementation",
                            "batch_size": batch_size
                        })
                    ),
                }
            }
            EngineImplementation::Candle(e) => e.embed_batch(texts),
        }
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        match &mut self.inner {
            EngineImplementation::Fast(e) => match e.embed_query(text) {
                Ok(vec) => Ok(vec),
                Err(err) => raise_error!(
                    "ERR_AI_ENGINE_FAST_QUERY_FAILED",
                    error = err,
                    context = json_value!({
                        "action": "single_query_dispatch",
                        "engine": "fast_cpu_implementation",
                        "text_length": text.len()
                    })
                ),
            },
            EngineImplementation::Candle(e) => e.embed_query(text),
        }
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    async fn test_default_engine_init() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json_value!({
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

    #[async_test]
    async fn test_engine_switching() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(
            &manager,
            "nlp",
            crate::utils::json::json_value!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        let mut fast_engine = EmbeddingEngine::new_with_type(EngineType::FastEmbed, &manager)
            .await
            .expect("FastEmbed init failed");
        let vec_fast = fast_engine.embed_query("Test Fast").expect("Embed failed");
        assert_eq!(
            vec_fast.len(),
            384,
            "FastEmbed (BGE-Small) doit sortir 384 dims"
        );

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
        }
    }
}
