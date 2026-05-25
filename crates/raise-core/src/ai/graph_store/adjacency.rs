// FICHIER : src-tauri/src/ai/graph_store/adjacency.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 LA FAÇADE GLOBALE

pub struct GraphAdjacency {
    pub uri_to_index: UnorderedMap<String, usize>,
    pub index_to_uri: Vec<String>,

    // 🎯 FORMAT SPARSE (COO) : Remplace la matrice dense [N, N]
    pub edge_src: NeuralTensor, // Tenseur 1D [E] des indices sources (u32)
    pub edge_dst: NeuralTensor, // Tenseur 1D [E] des indices cibles (u32)
}

impl GraphAdjacency {
    pub async fn build_from_store(
        manager: &CollectionsManager<'_>,
        device: &ComputeHardware,
    ) -> RaiseResult<Self> {
        let mut uri_map = UnorderedMap::new();
        let mut uri_vec = Vec::new();
        let mut documents = Vec::new();

        let mbse_collections = vec!["oa", "sa", "la", "pa", "epbs", "data", "transverse"];

        // 1. Découverte des nœuds
        for col_name in &mbse_collections {
            if let Ok(docs) = manager.list_all(col_name).await {
                for doc in docs {
                    if let Some(id) = doc.get("@id").and_then(|v| v.as_str()) {
                        uri_map.insert(id.to_string(), uri_vec.len());
                        uri_vec.push(id.to_string());
                        documents.push(doc);
                    }
                }
            }
        }

        let n = uri_vec.len();
        if n == 0 {
            raise_error!(
                "ERR_GNN_EMPTY_GRAPH",
                error = "Aucune entité Arcadia trouvée dans les collections MBSE.",
                context = json_value!({ "collections_scanned": mbse_collections })
            );
        }

        user_info!(
            "MSG_GNN_ADJACENCY_START",
            json_value!({ "nodes_count": n, "action": "build_sparse_topology" })
        );

        // 🎯 L'ALLOCATION SPARSE : Au lieu de vec![0.0; n*n], on liste juste les liens existants.
        let mut src_indices: Vec<u32> = Vec::new();
        let mut dst_indices: Vec<u32> = Vec::new();

        // Ajout des Self-loops obligatoires pour le GNN
        for i in 0..n {
            src_indices.push(i as u32);
            dst_indices.push(i as u32);
        }

        let arcadia_relations = ["realizes", "allocatedTo", "subComponents", "involvedActors"];

        // 2. Découverte des arêtes
        for (i, doc) in documents.iter().enumerate() {
            if let Some(obj) = doc.as_object() {
                for rel_key in arcadia_relations {
                    if let Some(value) = obj.get(rel_key) {
                        if let Some(arr) = value.as_array() {
                            for item in arr {
                                if let Some(tid) = item.get("@id").and_then(|v| v.as_str()) {
                                    if let Some(&j) = uri_map.get(tid) {
                                        src_indices.push(i as u32);
                                        dst_indices.push(j as u32);
                                    }
                                }
                            }
                        } else if let Some(tid) = value.get("@id").and_then(|v| v.as_str()) {
                            if let Some(&j) = uri_map.get(tid) {
                                src_indices.push(i as u32);
                                dst_indices.push(j as u32);
                            }
                        }
                    }
                }
            }
        }

        let edges_count = src_indices.len();
        let device_clone = device.clone();

        // 🎯 BOUCLIER CPU : Création des tenseurs 1D
        let tensor_result = os::execute_native_inference(move || {
            let t_src = match NeuralTensor::new(src_indices, &device_clone) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_GNN_TENSOR_SRC_FAILED", error = e.to_string()),
            };
            let t_dst = match NeuralTensor::new(dst_indices, &device_clone) {
                Ok(t) => t,
                Err(e) => raise_error!("ERR_GNN_TENSOR_DST_FAILED", error = e.to_string()),
            };
            Ok((t_src, t_dst))
        })
        .await;

        let (edge_src, edge_dst) = match tensor_result {
            Ok(res) => res,
            Err(e) => return Err(e),
        };

        user_success!(
            "MSG_GNN_ADJACENCY_READY",
            json_value!({ "nodes": n, "edges": edges_count, "format": "sparse_coo" })
        );

        Ok(Self {
            uri_to_index: uri_map,
            index_to_uri: uri_vec,
            edge_src,
            edge_dst,
        })
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_adjacency_build_with_arcadia_links_sparse() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        manager.create_collection("la", &schema_uri).await?;
        manager.create_collection("sa", &schema_uri).await?;

        let f1_doc = json_value!({ "_id": "F1", "@id": "la:F1", "realizes": [{ "@id": "sa:S1" }] });
        let s1_doc = json_value!({ "_id": "S1", "@id": "sa:S1" });

        manager.insert_raw("la", &f1_doc).await?;
        manager.insert_raw("sa", &s1_doc).await?;

        let adj = GraphAdjacency::build_from_store(&manager, &ComputeHardware::Cpu).await?;

        // 2 nœuds = 2 self-loops + 1 lien = 3 arêtes
        assert_eq!(adj.index_to_uri.len(), 2);

        let src_vec = match adj.edge_src.to_vec1::<u32>() {
            Ok(v) => v,
            Err(_) => panic!("Erreur conversion tenseur src"),
        };
        assert_eq!(src_vec.len(), 3, "Il devrait y avoir 3 arêtes (COO format)");

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_adjacency_error_on_empty_store() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let result = GraphAdjacency::build_from_store(&manager, &ComputeHardware::Cpu).await;

        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_GNN_EMPTY_GRAPH");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_GNN_EMPTY_GRAPH"),
        }
    }
}
