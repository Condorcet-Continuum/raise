// FICHIER : src-tauri/src/ai/nlp/embeddings/mod.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

pub mod fast;
pub mod native_nlp;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EngineType {
    Lightweight,
    Native,
}

pub struct EmbeddingEngine {
    inner: EngineImplementation,
}

enum EngineImplementation {
    Lightweight(Box<fast::FastEmbedEngine>),
    Native(Box<native_nlp::NativeNlpEngine>),
}

impl EmbeddingEngine {
    /// Initialise le moteur d'embeddings en respectant les points de montage système.
    /// Tente d'abord le moteur natif   avant de basculer sur Lightweight en cas d'échec.
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        user_info!(
            "MSG_NLP_ENGINE_INIT_START",
            json_value!({ "action": "attempt_native_nlp_init" })
        );

        // Tentative d'initialisation sur le moteur Native (Performance maximale)
        match Self::new_with_type(EngineType::Native, manager).await {
            Ok(engine) => Ok(engine),
            Err(e) => {
                // Bascule automatique vers Lightweight (CPU/ONNX) en cas d'échec matériel ou logiciel
                user_warn!(
                    "WRN_NLP_NATIVE_FALLBACK",
                    json_value!({
                        "error": e.to_string(),
                        "action": "fallback_to_fastembed",
                        "hint": "NLP Native indisponible (GPU/Poids manquants). Utilisation du backend CPU Lightweight."
                    })
                );
                Self::new_with_type(EngineType::Lightweight, manager).await
            }
        }
    }

    /// Initialise explicitement un type de moteur spécifique.
    pub async fn new_with_type(
        engine_type: EngineType,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        let inner = match engine_type {
            EngineType::Lightweight => {
                user_info!(
                    "MSG_NLP_ENGINE_TYPE_ACTIVE",
                    json_value!({ "type": "Lightweight", "backend": "ONNX/CPU" })
                );
                // 🎯 Match strict sur l'initialisation asynchrone
                let fast_engine = fast::FastEmbedEngine::new(manager).await?;
                EngineImplementation::Lightweight(Box::new(fast_engine))
            }
            EngineType::Native => {
                user_info!(
                    "MSG_NLP_ENGINE_TYPE_ACTIVE",
                    json_value!({ "type": "Native", "backend": "BERT/Native" })
                );
                // 🎯 Match strict sur l'initialisation asynchrone
                let native_engine = native_nlp::NativeNlpEngine::new(manager).await?;
                EngineImplementation::Native(Box::new(native_engine))
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
            EngineImplementation::Lightweight(e) => match e.embed_batch(texts) {
                Ok(res) => Ok(res),
                Err(err) => raise_error!(
                    "ERR_AI_ENGINE_FAST_BATCH_FAILED",
                    error = err.to_string(),
                    context = json_value!({ "batch_size": batch_size })
                ),
            },
            // Native gère ses propres erreurs sémantiques
            EngineImplementation::Native(e) => e.embed_batch(texts),
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
            EngineImplementation::Lightweight(e) => match e.embed_query(text) {
                Ok(vec) => Ok(vec),
                Err(err) => raise_error!(
                    "ERR_AI_ENGINE_FAST_QUERY_FAILED",
                    error = err.to_string(),
                    context = json_value!({ "text_len": text.len() })
                ),
            },
            EngineImplementation::Native(e) => e.embed_query(text),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience des Domaines)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Initialisation par défaut
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_default_engine_init() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut engine = EmbeddingEngine::new(&manager).await?;
        assert!(engine.embed_query("Hello").is_ok());
        Ok(())
    }

    /// Test existant : Commutation manuelle entre backends
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_engine_switching() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Test Lightweight
        let mut fast_engine =
            EmbeddingEngine::new_with_type(EngineType::Lightweight, &manager).await?;
        let vec_fast = fast_engine.embed_query("Test Lightweight")?;
        assert_eq!(vec_fast.len(), 384);

        // Test Native (si les poids de test sont présents ou mockés)
        if let Ok(mut native_engine) =
            EmbeddingEngine::new_with_type(EngineType::Native, &manager).await
        {
            let vec_native = native_engine.embed_query("Test Native")?;
            assert_eq!(vec_native.len(), 384);
        }

        Ok(())
    }

    /// Validation du Fallback automatique de la Façade
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_embedding_engine_fallback_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 SABOTAGE CIBLÉ : On altère la config dans la VRAIE partition Système
        // pour faire croire que le modèle lourd (Native) a été supprimé du disque.
        let mut nlp_doc = manager
            .get_document("service_configs", "cfg_ai_nlp_test")
            .await?
            .expect("La config NLP devrait être présente");

        nlp_doc["service_settings"]["rust_safetensors_file"] = json_value!("ghost.safetensors");
        let _ = manager
            .delete_document("service_configs", "cfg_ai_nlp_test")
            .await;
        manager.insert_raw("service_configs", &nlp_doc).await?;

        // 🎯 MAGIE : On appelle la façade globale `EmbeddingEngine::new`
        // Elle va tenter d'allumer Native, se prendre le mur du fichier manquant,
        // attraper l'erreur gracieusement, et s'allumer en Lightweight !
        let mut engine = EmbeddingEngine::new(&manager).await?;

        // 🟢 ZÉRO DETTE : On vérifie que le moteur de secours fonctionne
        let result = engine.embed_query("Test de résilience du fallback");
        assert!(
            result.is_ok(),
            "La façade aurait dû basculer en mode Lightweight et survivre."
        );

        // Le vecteur Lightweight (FastEmbed) a souvent une dimension différente (ex: 384)
        assert!(!result.unwrap().is_empty());

        Ok(())
    }

    ///  Protection contre les requêtes vides
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_engine_query_validation() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut engine = EmbeddingEngine::new_with_type(EngineType::Lightweight, &manager).await?;

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
