// FICHIER : src-tauri/src/ai/nlp/embeddings/mod.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

pub mod candle;
pub mod fast;

#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Initialise le moteur d'embeddings en respectant les points de montage système.
    /// Tente d'abord le moteur natif (Candle) avant de basculer sur FastEmbed en cas d'échec.
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        user_info!(
            "MSG_NLP_ENGINE_INIT_START",
            json_value!({ "action": "attempt_native_candle_init" })
        );

        // Tentative d'initialisation sur le moteur Candle (Performance maximale)
        match Self::new_with_type(EngineType::Candle, manager).await {
            Ok(engine) => Ok(engine),
            Err(e) => {
                // Bascule automatique vers FastEmbed (CPU/ONNX) en cas d'échec matériel ou logiciel
                user_warn!(
                    "WRN_NLP_CANDLE_FALLBACK",
                    json_value!({
                        "error": e.to_string(),
                        "action": "fallback_to_fastembed",
                        "hint": "Candle indisponible (GPU/Poids manquants). Utilisation du backend CPU FastEmbed."
                    })
                );
                Self::new_with_type(EngineType::FastEmbed, manager).await
            }
        }
    }

    /// Initialise explicitement un type de moteur spécifique.
    pub async fn new_with_type(
        engine_type: EngineType,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        let inner = match engine_type {
            EngineType::FastEmbed => {
                user_info!(
                    "MSG_NLP_ENGINE_TYPE_ACTIVE",
                    json_value!({ "type": "FastEmbed", "backend": "ONNX/CPU" })
                );
                // 🎯 Match strict sur l'initialisation asynchrone
                let fast_engine = fast::FastEmbedEngine::new(manager).await?;
                EngineImplementation::Fast(Box::new(fast_engine))
            }
            EngineType::Candle => {
                user_info!(
                    "MSG_NLP_ENGINE_TYPE_ACTIVE",
                    json_value!({ "type": "Candle", "backend": "BERT/Native" })
                );
                // 🎯 Match strict sur l'initialisation asynchrone
                let candle_engine = candle::CandleEngine::new(manager).await?;
                EngineImplementation::Candle(Box::new(candle_engine))
            }
        };
        Ok(Self { inner })
    }

    /// Vectorise un lot de textes (Batch Inference) avec dispatching sémantique.
    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();
        if batch_size == 0 {
            return Ok(Vec::new());
        }

        match &mut self.inner {
            EngineImplementation::Fast(e) => match e.embed_batch(texts) {
                Ok(res) => Ok(res),
                Err(err) => raise_error!(
                    "ERR_AI_ENGINE_FAST_BATCH_FAILED",
                    error = err.to_string(),
                    context = json_value!({ "batch_size": batch_size })
                ),
            },
            // Candle gère ses propres erreurs sémantiques
            EngineImplementation::Candle(e) => e.embed_batch(texts),
        }
    }

    /// Vectorise une requête unique.
    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        if text.is_empty() {
            raise_error!(
                "ERR_NLP_QUERY_EMPTY",
                error = "Impossible de vectoriser une chaîne vide."
            );
        }

        match &mut self.inner {
            EngineImplementation::Fast(e) => match e.embed_query(text) {
                Ok(vec) => Ok(vec),
                Err(err) => raise_error!(
                    "ERR_AI_ENGINE_FAST_QUERY_FAILED",
                    error = err.to_string(),
                    context = json_value!({ "text_len": text.len() })
                ),
            },
            EngineImplementation::Candle(e) => e.embed_query(text),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience des Domaines)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    /// Test existant : Initialisation par défaut
    #[async_test]
    async fn test_default_engine_init() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm"})).await;

        let mut engine = EmbeddingEngine::new(&manager).await?;
        assert!(engine.embed_query("Hello").is_ok());
        Ok(())
    }

    /// Test existant : Commutation manuelle entre backends
    #[async_test]
    async fn test_engine_switching() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm"})).await;

        // Test FastEmbed
        let mut fast_engine =
            EmbeddingEngine::new_with_type(EngineType::FastEmbed, &manager).await?;
        let vec_fast = fast_engine.embed_query("Test Fast")?;
        assert_eq!(vec_fast.len(), 384);

        // Test Candle (si les poids de test sont présents ou mockés)
        if let Ok(mut candle_engine) =
            EmbeddingEngine::new_with_type(EngineType::Candle, &manager).await
        {
            let vec_candle = candle_engine.embed_query("Test Candle")?;
            assert_eq!(vec_candle.len(), 384);
        }

        Ok(())
    }

    /// On teste l'initialisation directe pour valider l'interception d'erreur
    #[async_test]
    async fn test_engine_resilience_bad_domain() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;

        // Manager pointant sur un domaine inexistant
        let manager = CollectionsManager::new(&sandbox.db, "ghost_zone", "ghost_db");

        // 🎯 On appelle new_with_type(Candle) directement
        // Cela évite le fallback automatique de EmbeddingEngine::new()
        // et permet de vérifier que la couche DB lève bien l'erreur attendue.
        let result = EmbeddingEngine::new_with_type(EngineType::Candle, &manager).await;

        match result {
        Err(AppError::Structured(_)) => Ok(()),
        _ => panic!("Le moteur aurait dû lever une erreur structurée lors de l'accès au domaine 'ghost_zone'"),
    }
    }

    /// 🎯 NOUVEAU TEST : Protection contre les requêtes vides
    #[async_test]
    async fn test_engine_query_validation() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "nlp", json_value!({})).await;
        let mut engine = EmbeddingEngine::new_with_type(EngineType::FastEmbed, &manager).await?;

        let result = engine.embed_query("");
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_NLP_QUERY_EMPTY");
                Ok(())
            }
            _ => panic!("La requête vide aurait dû être rejetée"),
        }
    }
}
