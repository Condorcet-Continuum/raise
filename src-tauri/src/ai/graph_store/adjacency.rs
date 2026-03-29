// FICHIER : src-tauri/src/ai/graph_store/adjacency.rs
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;
use candle_core::{Device, Tensor};

/// Traduit l'ontologie Arcadia en structure mathématique (Matrice A) pour le GNN.
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
        // On ignore les configs, les agents, et les logs. On ne garde que l'ingénierie.
        let mbse_collections = vec!["oa", "sa", "la", "pa", "epbs", "data", "transverse"];

        // 1. PHASE DE DÉCOUVERTE : Récupération ciblée
        for col_name in mbse_collections {
            // Si la collection n'existe pas encore, list_all échoue silencieusement, ce qui est voulu.
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
                error = "Aucune entité Arcadia trouvée dans les collections MBSE via le CollectionsManager."
            );
        }

        user_info!(
            "🕸️ [GNN] Matrice d'adjacence en construction pour {} nœuds MBSE...",
            json_value!(n)
        );

        // 2. PHASE DE CONSTRUCTION : Matrice A + I
        let mut data = vec![0.0f32; n * n];

        // Self-loops (Diagonale à 1) pour que le GNN n'oublie pas les caractéristiques du nœud lui-même
        for i in 0..n {
            data[i * n + i] = 1.0;
        }

        // 🎯 OPTIMISATION 2 : Ciblage strict des relations Arcadia (O(1) sur les propriétés)
        let arcadia_relations = ["realizes", "allocatedTo", "subComponents", "involvedActors"];

        for (i, doc) in documents.iter().enumerate() {
            if let Some(obj) = doc.as_object() {
                for rel_key in arcadia_relations {
                    if let Some(value) = obj.get(rel_key) {
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
                "ERR_GNN_TENSOR_ADJ",
                error = e,
                context = json_value!({ "nodes_count": n })
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
// TESTS UNITAIRES (VALIDATION TOPOLOGIQUE MBSE VIA MANAGER)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_adjacency_build_with_arcadia_links() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        manager.init_db().await.unwrap();

        // Création des collections MBSE virtuelles
        manager
            .create_collection(
                "la",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .create_collection(
                "sa",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 🎯 On utilise l'API de base de données (Manager)
        let f1_doc = json_value!({ "_id": "F1", "@id": "la:F1", "realizes": [{ "@id": "sa:S1" }] });
        let s1_doc = json_value!({ "_id": "S1", "@id": "sa:S1" });

        manager.insert_raw("la", &f1_doc).await.unwrap();
        manager.insert_raw("sa", &s1_doc).await.unwrap();

        let device = Device::Cpu;
        let adj_res = GraphAdjacency::build_from_store(&manager, &device).await;

        assert!(
            adj_res.is_ok(),
            "La construction de la matrice ne doit pas échouer."
        );
        let adj = adj_res.unwrap();

        assert_eq!(
            adj.index_to_uri.len(),
            2,
            "Le graphe devrait contenir exactement 2 nœuds MBSE."
        );

        let data = adj.matrix.flatten_all().unwrap().to_vec1::<f32>().unwrap();
        let i = adj.uri_to_index["la:F1"];
        let j = adj.uri_to_index["sa:S1"];

        assert_eq!(
            data[i * 2 + j],
            1.0,
            "Le lien sémantique la:F1 -> sa:S1 ('realizes') est manquant."
        );
        assert_eq!(
            data[i * 2 + i],
            1.0,
            "La boucle identité (self-loop) est manquante."
        );
    }
}
