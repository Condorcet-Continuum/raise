// FICHIER : src-tauri/src/ai/graph_store/adjacency.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE
use candle_core::{Device, Tensor};

/// Traduit l'ontologie Arcadia en structure mathématique (Matrice A) pour le GNN.
/// Gère la correspondance entre les URI sémantiques et les indices de tenseurs.
pub struct GraphAdjacency {
    pub uri_to_index: UnorderedMap<String, usize>,
    pub index_to_uri: Vec<String>,
    pub matrix: Tensor,
}

impl GraphAdjacency {
    /// Construit la matrice d'adjacence de manière asynchrone via le CollectionsManager.
    pub async fn build_from_store(
        manager: &CollectionsManager<'_>,
        device: &Device,
    ) -> RaiseResult<Self> {
        let mut uri_map = UnorderedMap::new();
        let mut uri_vec = Vec::new();
        let mut documents = Vec::new();

        // 🎯 OPTIMISATION 1 : Filtrage Ontologique (O(1) sur les collections)
        let mbse_collections = vec!["oa", "sa", "la", "pa", "epbs", "data", "transverse"];

        // 1. PHASE DE DÉCOUVERTE : Récupération ciblée via le manager
        // Utilisation d'une référence pour éviter le move du vecteur dans les logs d'erreurs
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
            json_value!({ "nodes_count": n, "action": "build_matrix" })
        );

        // 2. PHASE DE CONSTRUCTION : Matrice d'adjacence A + Boucle Identité (I)
        let mut data = vec![0.0f32; n * n];

        // Self-loops (Diagonale à 1) : crucial pour la propagation GNN.
        for i in 0..n {
            data[i * n + i] = 1.0;
        }

        // 🎯 OPTIMISATION 2 : Ciblage strict des relations Arcadia
        let arcadia_relations = ["realizes", "allocatedTo", "subComponents", "involvedActors"];

        for (i, doc) in documents.iter().enumerate() {
            if let Some(obj) = doc.as_object() {
                for rel_key in arcadia_relations {
                    if let Some(value) = obj.get(rel_key) {
                        // Gestion polymorphique : Relation unique ou tableau de relations
                        if let Some(arr) = value.as_array() {
                            for item in arr {
                                if let Some(tid) = item.get("@id").and_then(|v| v.as_str()) {
                                    if let Some(&j) = uri_map.get(tid) {
                                        data[i * n + j] = 1.0;
                                    }
                                }
                            }
                        } else if let Some(tid) = value.get("@id").and_then(|v| v.as_str()) {
                            if let Some(&j) = uri_map.get(tid) {
                                data[i * n + j] = 1.0;
                            }
                        }
                    }
                }
            }
        }

        // 3. TRANSFERT HARDWARE : Conversion vers Tenseur Candle
        let matrix = match Tensor::from_vec(data, (n, n), device) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_GNN_TENSOR_ADJ_FAILED",
                error = e.to_string(),
                context = json_value!({ "nodes_count": n, "device": format!("{:?}", device) })
            ),
        };

        Ok(Self {
            uri_to_index: uri_map,
            index_to_uri: uri_vec,
            matrix,
        })
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Topologique MBSE & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    /// Test existant : Validation des liens sémantiques Arcadia (LA -> SA)
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_adjacency_build_with_arcadia_links() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
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

        let device = Device::Cpu;
        let adj = GraphAdjacency::build_from_store(&manager, &device).await?;

        assert_eq!(adj.index_to_uri.len(), 2);

        // Extraction et validation avec Match
        let flat = match adj.matrix.flatten_all() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TEST_TENSOR", error = e.to_string()),
        };

        let data = match flat.to_vec1::<f32>() {
            Ok(v) => v,
            Err(e) => raise_error!("ERR_TEST_VEC", error = e.to_string()),
        };

        let i = match adj.uri_to_index.get("la:F1") {
            Some(&idx) => idx,
            None => panic!("Index F1 manquant"),
        };
        let j = match adj.uri_to_index.get("sa:S1") {
            Some(&idx) => idx,
            None => panic!("Index S1 manquant"),
        };

        assert_eq!(data[i * 2 + j], 1.0, "Lien LA -> SA manquant");
        assert_eq!(data[i * 2 + i], 1.0, "Self-loop i manquante");

        Ok(())
    }

    /// Test de résilience : Graphe MBSE vide
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_adjacency_error_on_empty_store() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let result = GraphAdjacency::build_from_store(&manager, &Device::Cpu).await;

        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_GNN_EMPTY_GRAPH");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_GNN_EMPTY_GRAPH"),
        }
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une configuration de Mount Point erronée
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_adjacency_resilience_bad_mount_point() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        // Manager pointant sur une partition fantôme
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        let result = GraphAdjacency::build_from_store(&manager, &Device::Cpu).await;

        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_GNN_EMPTY_GRAPH");
                Ok(())
            }
            _ => panic!("Le build aurait dû échouer proprement sur un mount point vide"),
        }
    }

    /// 🎯 NOUVEAU TEST : Résilience sur liens rompus (URI cible inexistante)
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_adjacency_resilience_on_broken_links() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
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

        let doc = json_value!({ "_id": "F2", "@id": "la:F2", "realizes": [{ "@id": "sa:GHOST" }] });
        manager.insert_raw("la", &doc).await?;

        let adj = GraphAdjacency::build_from_store(&manager, &Device::Cpu).await?;

        // Doit réussir mais ignorer GHOST
        assert_eq!(adj.index_to_uri.len(), 1);
        Ok(())
    }
}
