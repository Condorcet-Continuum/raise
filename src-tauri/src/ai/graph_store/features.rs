// FICHIER : src-tauri/src/ai/graph_store/features.rs

use crate::ai::nlp::embeddings::EmbeddingEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique
use candle_core::{Device, Tensor};

pub struct GraphFeatures {
    /// Le tenseur des caractéristiques [N, D] (H-Matrix)
    pub matrix: Tensor,
}

impl GraphFeatures {
    /// Construit la matrice H en vectorisant chaque nœud par lots (Batching) pour
    /// maximiser les performances GPU/CPU. Extraction sémantique profonde via JSON-DB.
    pub async fn build_from_store(
        manager: &CollectionsManager<'_>, // 🎯 SSOT: Manager pour accès aux documents
        index_to_uri: &[String],
        embedding_engine: &mut EmbeddingEngine,
        device: &Device,
    ) -> RaiseResult<Self> {
        let n_nodes = index_to_uri.len();
        if n_nodes == 0 {
            raise_error!(
                "ERR_GNN_EMPTY_FEATURES",
                error =
                    "La liste des URIs est vide. Impossible de construire la matrice de features."
            );
        }

        user_info!(
            "MSG_GNN_FEATURES_BATCH_START",
            json_value!({ "nodes_count": n_nodes, "action": "deep_semantic_extraction" })
        );

        let mut texts_to_embed: Vec<String> = Vec::with_capacity(n_nodes);

        // 🎯 OPTIMISATION 1 : Récupération du contexte métier réel via Match
        for uri in index_to_uri {
            let mut semantic_text = String::new();

            // L'URI suit le pattern "collection:id" (ex: "la:F1")
            let parts: Vec<&str> = uri.split(':').collect();
            if parts.len() >= 2 {
                let col = parts[0];
                let id = parts[1];

                // Tentative de récupération sémantique via le manager
                if let Ok(Some(doc)) = manager.get_document(col, id).await {
                    semantic_text =
                        crate::ai::graph_store::store::extract_rich_semantic_content(&doc);
                } else if let Ok(Some(doc)) = manager.get_document(col, uri).await {
                    // Fallback si l'identifiant inclut le préfixe dans la DB
                    semantic_text =
                        crate::ai::graph_store::store::extract_rich_semantic_content(&doc);
                }
            }

            // Fallback ultime : on utilise l'URI formatée si le document est manquant
            if semantic_text.trim().is_empty() {
                semantic_text = uri.replace([':', '_'], " ");
            }

            texts_to_embed.push(semantic_text);
        }

        // 🎯 OPTIMISATION 2 : Inférence par lot (Batch Inference) avec gestion d'erreur
        let batch_vectors = match embedding_engine.embed_batch(texts_to_embed) {
            Ok(v) => v,
            Err(e) => {
                raise_error!(
                    "ERR_GNN_EMBEDDING_BATCH_GEN",
                    error = e.to_string(),
                    context = json_value!({ "batch_size": n_nodes })
                );
            }
        };

        // 🎯 OPTIMISATION 3 : Validation des dimensions et pré-allocation
        let expected_dim = match batch_vectors.first() {
            Some(v) => v.len(),
            None => raise_error!(
                "ERR_GNN_EMBEDDING_EMPTY",
                error = "Le moteur NLP a renvoyé un lot vide."
            ),
        };

        let mut all_embeddings_data: Vec<f32> = Vec::with_capacity(n_nodes * expected_dim);

        for (i, vector) in batch_vectors.into_iter().enumerate() {
            if vector.len() != expected_dim {
                raise_error!(
                    "ERR_GNN_DIMENSION_MISMATCH",
                    error = "Incohérence des dimensions d'embedding dans le lot.",
                    context = json_value!({
                        "expected": expected_dim,
                        "got": vector.len(),
                        "uri": index_to_uri[i]
                    })
                );
            }
            all_embeddings_data.extend(vector);
        }

        // 3. Création du Tenseur final [N, D] sur le matériel configuré
        let matrix = match Tensor::from_vec(all_embeddings_data, (n_nodes, expected_dim), device) {
            Ok(t) => t,
            Err(e) => {
                raise_error!(
                    "ERR_GNN_FEATURES_TENSOR_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "nodes": n_nodes, "dim": expected_dim, "device": format!("{:?}", device) })
                );
            }
        };

        user_success!(
            "MSG_GNN_FEATURES_READY",
            json_value!({ "shape": format!("[{}, {}]", n_nodes, expected_dim) })
        );

        Ok(Self { matrix })
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Sémantique Batch & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_graph_features_generation_batch_mode() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du point de montage système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        AgentDbSandbox::mock_db(&manager).await?;

        // Initialisation des collections MBSE
        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        for col in &["la", "sa", "pa"] {
            manager.create_collection(col, &schema_uri).await?;
        }

        manager
            .insert_raw(
                "la",
                &json_value!({"_id": "F1", "name": "Radar", "description": "Detection"}),
            )
            .await?;
        manager
            .insert_raw("sa", &json_value!({"_id": "S1", "name": "Defense"}))
            .await?;
        manager
            .insert_raw("pa", &json_value!({"_id": "H1", "name": "Antenna"}))
            .await?;

        inject_mock_component(
            &manager,
            "nlp",
            json_value!({
                "model_name": "minilm",
                "rust_config_file": "config.json",
                "rust_tokenizer_file": "tokenizer.json",
                "rust_safetensors_file": "model.safetensors"
            }),
        )
        .await;

        let mut engine = EmbeddingEngine::new(&manager).await?;
        let uris = vec![
            "la:F1".to_string(),
            "sa:S1".to_string(),
            "pa:H1".to_string(),
        ];

        let feat =
            GraphFeatures::build_from_store(&manager, &uris, &mut engine, &Device::Cpu).await?;

        assert_eq!(
            feat.matrix.dims(),
            &[3, 384],
            "La matrice H devrait être [3, 384] (MiniLM)"
        );
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_features_empty_list_fails() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm"})).await;
        let mut engine = EmbeddingEngine::new(&manager).await?;

        let res = GraphFeatures::build_from_store(&manager, &[], &mut engine, &Device::Cpu).await;

        match res {
            Err(AppError::Structured(err)) => assert_eq!(err.code, "ERR_GNN_EMPTY_FEATURES"),
            _ => panic!("Le moteur aurait dû lever ERR_GNN_EMPTY_FEATURES"),
        }
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience sur documents manquants (Fallback)
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_features_fallback_on_missing_docs() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "nlp", json_value!({"model_name": "minilm"})).await;
        let mut engine = EmbeddingEngine::new(&manager).await?;

        // URI pointant vers un document inexistant
        let uris = vec!["ghost:entity_01".to_string()];

        let feat =
            GraphFeatures::build_from_store(&manager, &uris, &mut engine, &Device::Cpu).await?;

        // Le build doit réussir via le fallback textuel (URI formatée)
        assert_eq!(feat.matrix.dims(), &[1, 384]);
        Ok(())
    }
}
