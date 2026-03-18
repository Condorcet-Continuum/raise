// FICHIER : src-tauri/src/ai/graph_store/features.rs
use crate::ai::nlp::embeddings::EmbeddingEngine;
use crate::utils::prelude::*;
use candle_core::{Device, Tensor};

pub struct GraphFeatures {
    /// Le tenseur des caractéristiques [N, D]
    pub matrix: Tensor,
}

impl GraphFeatures {
    /// Construit la matrice H en vectorisant chaque nœud par lots (Batching) pour
    /// maximiser les performances GPU/CPU.
    pub async fn build_from_store(
        index_to_uri: &[String],
        embedding_engine: &mut EmbeddingEngine,
        device: &Device,
    ) -> RaiseResult<Self> {
        let n_nodes = index_to_uri.len();
        if n_nodes == 0 {
            raise_error!(
                "ERR_GNN_EMPTY_FEATURES",
                error = "La liste des URIs est vide."
            );
        }

        user_info!(
            "🧠 [GNN] Extraction des caractéristiques en mode BATCH pour {} nœuds...",
            json_value!(n_nodes)
        );

        // 🎯 Clippy FIX : Remplacement optimisé en une seule passe d'allocation
        let texts_to_embed: Vec<String> = index_to_uri
            .iter()
            .map(|uri| uri.replace([':', '_'], " "))
            .collect();

        // 🎯 OPTIMISATION PROD 2 : Inférence par lot (Batch Inference)
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

        // 🎯 OPTIMISATION PROD 3 : Pré-allocation mémoire intelligente
        let expected_dim = batch_vectors.first().map_or(0, |v| v.len());
        let mut all_embeddings_data: Vec<f32> = Vec::with_capacity(n_nodes * expected_dim);

        for (i, vector) in batch_vectors.into_iter().enumerate() {
            if vector.len() != expected_dim {
                raise_error!(
                    "ERR_GNN_DIMENSION_MISMATCH",
                    error = "Incohérence des dimensions d'embedding détectée dans le lot.",
                    context = json_value!({
                        "expected": expected_dim,
                        "got": vector.len(),
                        "uri": index_to_uri[i]
                    })
                );
            }
            all_embeddings_data.extend(vector);
        }

        // 3. Création du Tenseur final [N, D]
        let matrix = match Tensor::from_vec(all_embeddings_data, (n_nodes, expected_dim), device) {
            Ok(t) => t,
            Err(e) => {
                raise_error!(
                    "ERR_GNN_FEATURES_TENSOR",
                    error = e,
                    context = json_value!({ "nodes": n_nodes, "dim": expected_dim })
                );
            }
        };

        user_success!(
            "✅ [GNN] Matrice H construite (Mode Batch accéléré).",
            json_value!({ "shape": format!("[{}, {}]", n_nodes, expected_dim) })
        );

        Ok(Self { matrix })
    }
}

// =========================================================================
// TESTS UNITAIRES (VALIDATION SÉMANTIQUE BATCH)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    #[async_test]
    async fn test_graph_features_generation_batch_mode() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

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

        if let Ok(mut engine) = EmbeddingEngine::new(&manager).await {
            let uris = vec![
                "la:Function_A".to_string(),
                "sa:System_B".to_string(),
                "pa:Hardware_C".to_string(),
            ];
            let device = Device::Cpu;

            let feat_res = GraphFeatures::build_from_store(&uris, &mut engine, &device).await;

            assert!(
                feat_res.is_ok(),
                "La génération des features en batch a échoué."
            );
            let feat = feat_res.unwrap();

            assert_eq!(
                feat.matrix.dims(),
                &[3, 384], // 3 nœuds, 384 dimensions
                "La matrice H devrait avoir 3 lignes et 384 colonnes (MiniLM)."
            );
        }
    }

    #[async_test]
    async fn test_features_empty_list_fails() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

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

        if let Ok(mut engine) = EmbeddingEngine::new(&manager).await {
            let res = GraphFeatures::build_from_store(&[], &mut engine, &Device::Cpu).await;
            assert!(res.is_err());
            if let Err(AppError::Structured(err)) = res {
                assert_eq!(err.code, "ERR_GNN_EMPTY_FEATURES");
            }
        }
    }
}
