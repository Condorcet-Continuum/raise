// FICHIER : src-tauri/src/ai/memory/candle_store.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use super::{MemoryRecord, VectorStore};
use candle_core::{Device, Tensor};

/// Store vectoriel local RAISE agissant comme un index "Deep Learning"
/// pour les collections de données gérées par JSON-DB.
pub struct CandleLocalStore {
    device: Device,
    /// Gestion isolée par Collection (Map)
    state: AsyncRwLock<UnorderedMap<String, CollectionState>>,
}

#[derive(Default, Clone)]
struct CollectionState {
    /// Lien direct : Ligne de la matrice -> `_id` du document dans JSON-DB
    index_to_id: Vec<String>,
    vector_matrix: Option<Tensor>,
}

impl CandleLocalStore {
    /// Initialise le store vectoriel.
    /// (Le paramètre _path est ignoré en faveur de la résolution dynamique via JSON-DB)
    pub fn new(_path: &Path, device: &Device) -> Self {
        Self {
            device: device.clone(),
            state: AsyncRwLock::new(UnorderedMap::new()),
        }
    }

    /// 🎯 RÉSOLUTION DÉTERMINISTE : Les tenseurs mémoires sont rangés au même endroit que les modèles DL !
    async fn get_tensor_dir(manager: &CollectionsManager<'_>, col: &str) -> PathBuf {
        manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("tensors")
            .join(col)
    }

    /// 🎯 LAZY LOADING : Charge les tenseurs depuis le SSD vers la VRAM uniquement lorsque nécessaire
    async fn ensure_loaded(&self, manager: &CollectionsManager<'_>, col: &str) -> RaiseResult<()> {
        {
            let state = self.state.read().await;
            if state.contains_key(col) {
                return Ok(());
            }
        }
        let mut state = self.state.write().await;
        if state.contains_key(col) {
            return Ok(());
        }

        let col_dir = Self::get_tensor_dir(manager, col).await;
        let index_path = col_dir.join("index.json");
        let tensor_path = col_dir.join("vectors.safetensors");

        let mut index_to_id = Vec::new();
        let mut matrix = None;

        if fs::exists_async(&index_path).await && fs::exists_async(&tensor_path).await {
            index_to_id = fs::read_json_async(&index_path).await.unwrap_or_default();
            if !index_to_id.is_empty() {
                let mut tensors =
                    candle_core::safetensors::load(&tensor_path, &self.device).unwrap_or_default();
                matrix = tensors.remove("vectors");
            }
        }

        state.insert(
            col.to_string(),
            CollectionState {
                index_to_id,
                vector_matrix: matrix,
            },
        );

        Ok(())
    }

    /// Sauvegarde atomique de la matrice tensorielle de la collection.
    async fn save_collection(
        &self,
        manager: &CollectionsManager<'_>,
        col: &str,
        col_state: &CollectionState,
    ) -> RaiseResult<()> {
        let col_dir = Self::get_tensor_dir(manager, col).await;
        fs::ensure_dir_async(&col_dir).await?;

        // Sauvegarde de la table de routage (Index de matrice -> Document ID)
        let index_path = col_dir.join("index.json");
        fs::write_json_atomic_async(&index_path, &col_state.index_to_id).await?;

        // Sauvegarde physique accélérée du Cerveau Vectoriel
        if let Some(ref matrix) = col_state.vector_matrix {
            let tensor_path = col_dir.join("vectors.safetensors");
            let matrix_clone = matrix.clone();
            let thread_path = tensor_path.clone();

            let join_handle = spawn_cpu_task(move || {
                let mut map = UnorderedMap::new();
                map.insert("vectors".to_string(), matrix_clone);
                candle_core::safetensors::save(&map, thread_path)
            })
            .await;

            if let Err(e) = join_handle {
                raise_error!("ERR_THREAD_PANIC_DURING_SAVE", error = e.to_string());
            } else if let Ok(Err(e)) = join_handle {
                raise_error!("ERR_AI_SAFE_TENSORS_SAVE_FAILED", error = e.to_string());
            }
        }
        Ok(())
    }

    // Les anciennes méthodes globales ne sont plus nécessaires, l'état est autogéré par collection
    pub async fn save(&self) -> RaiseResult<()> {
        Ok(())
    }
    pub async fn load(&self) -> RaiseResult<()> {
        Ok(())
    }
}

#[async_interface]
impl VectorStore for CandleLocalStore {
    async fn init_collection(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        _size: u64,
    ) -> RaiseResult<()> {
        let col_dir = Self::get_tensor_dir(manager, collection_name).await;
        fs::ensure_dir_async(&col_dir).await?;

        // Sécurisation : on s'assure que la collection existe bien côté JSON-DB
        let _ = manager
            .create_collection(
                collection_name,
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;

        self.ensure_loaded(manager, collection_name).await?;
        Ok(())
    }

    /// Ajout vectoriel ET Documentaire synchronisé
    async fn add_documents(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        records: Vec<MemoryRecord>,
    ) -> RaiseResult<()> {
        self.ensure_loaded(manager, collection_name).await?;

        let mut valid_vectors = Vec::new();
        let mut new_ids = Vec::new();

        for rec in records {
            let id = if rec.id.is_empty() {
                UniqueId::new_v4().to_string()
            } else {
                rec.id.clone()
            };

            // 🎯 1. LA MAGIE DU GRAPHE : La donnée textuelle est confiée au JSON-DB
            let doc = json_value!({
                "_id": id.clone(),
                "content": rec.content,
                "metadata": rec.metadata
            });
            manager.upsert_document(collection_name, doc).await?;

            // 2. Préparation pour le Moteur Tensoriel
            if let Some(vec) = rec.vectors {
                valid_vectors.push(vec);
                new_ids.push(id);
            }
        }

        if valid_vectors.is_empty() {
            return Ok(());
        }

        // 🎯 3. Calcul GPU/CPU : Concaténation de la matrice existante avec les nouveaux lots
        let mut state = self.state.write().await;
        let col_state = state
            .entry(collection_name.to_string())
            .or_insert_with(CollectionState::default);

        let n_new = valid_vectors.len();
        let d = valid_vectors[0].len();
        let flat_new: Vec<f32> = valid_vectors.into_iter().flatten().collect();

        let new_tensor = match Tensor::from_vec(flat_new, (n_new, d), &self.device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_VECTOR_CREATION_FAILED", error = e.to_string()),
        };

        col_state.vector_matrix = match &col_state.vector_matrix {
            Some(existing) => match Tensor::cat(&[existing, &new_tensor], 0) {
                Ok(t) => Some(t),
                Err(e) => raise_error!("ERR_VECTOR_CONCAT_FAILED", error = e.to_string()),
            },
            None => Some(new_tensor),
        };
        col_state.index_to_id.extend(new_ids);

        // 4. Auto-sauvegarde déterministe pour éviter les pertes de contexte
        self.save_collection(manager, collection_name, col_state)
            .await?;

        Ok(())
    }

    /// Recherche Hybride : Tensor Matching (Candle) + Metadata Filtering (JSON-DB)
    async fn search_similarity(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        query_vec: &[f32],
        limit: u64,
        threshold: f32,
        filter: Option<UnorderedMap<String, String>>,
    ) -> RaiseResult<Vec<MemoryRecord>> {
        self.ensure_loaded(manager, collection_name).await?;

        let state = self.state.read().await;
        let col_state = match state.get(collection_name) {
            Some(cs) => cs,
            None => return Ok(vec![]),
        };

        let matrix = match &col_state.vector_matrix {
            Some(m) => m,
            None => return Ok(vec![]),
        };

        // 1. Calcul ultra-rapide des similarités (Produit Scalaire)
        let q = match Tensor::from_slice(query_vec, (1, query_vec.len()), &self.device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_VECTOR_QUERY_INIT", error = e.to_string()),
        };

        let q_transposed = match q.t() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_VECTOR_TRANSPOSE", error = e.to_string()),
        };

        let scores_tensor = match matrix.matmul(&q_transposed) {
            Ok(res) => res,
            Err(e) => raise_error!("ERR_VECTOR_MATMUL", error = e.to_string()),
        };

        let scores = match scores_tensor.flatten_all().and_then(|t| t.to_vec1::<f32>()) {
            Ok(v) => v,
            Err(e) => raise_error!("ERR_VECTOR_FLATTEN", error = e.to_string()),
        };

        // 2. Filtrage mathématique initial
        let mut ranked: Vec<(f32, usize)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, score)| *score >= threshold)
            .map(|(i, s)| (s, i))
            .collect();

        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(FmtOrdering::Equal));

        let mut results = Vec::new();

        // 3. Hydratation DB + Filtrage Métadonnées
        for (_, idx) in ranked {
            if results.len() >= limit as usize {
                break;
            }

            let id = &col_state.index_to_id[idx];

            // 🎯 Le Graphe de Connaissance valide et hydrate la donnée mémoire
            if let Ok(Some(doc)) = manager.get_document(collection_name, id).await {
                let mut meta_match = true;

                if let Some(ref f_map) = filter {
                    let doc_meta = doc.get("metadata").and_then(|m| m.as_object());
                    for (k, v) in f_map {
                        let val_match = doc_meta
                            .and_then(|m| m.get(k))
                            .map(|val| {
                                if let Some(s) = val.as_str() {
                                    s == v
                                } else if let Some(n) = val.as_i64() {
                                    v.parse::<i64>().is_ok_and(|p| p == n)
                                } else if let Some(b) = val.as_bool() {
                                    v.parse::<bool>().is_ok_and(|p| p == b)
                                } else {
                                    false
                                }
                            })
                            .unwrap_or(false);

                        if !val_match {
                            meta_match = false;
                            break;
                        }
                    }
                }

                if meta_match {
                    results.push(MemoryRecord {
                        id: id.clone(),
                        content: doc
                            .get("content")
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string(),
                        metadata: doc.get("metadata").cloned().unwrap_or(json_value!({})),
                        vectors: None, // Inutile de surcharger la RAM du front avec des milliers de float32
                    });
                }
            }
        }

        Ok(results)
    }
}

// --- TESTS UNITAIRES HYPER ROBUSTES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_full_search_with_metadata_filter() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        AgentDbSandbox::mock_db(&manager).await.unwrap();

        // Le path passé ici est dorénavant ignoré, la db gère tout
        let store = CandleLocalStore::new(&sandbox.domain_root, &Device::Cpu);

        let col = "tech";
        store.init_collection(&manager, col, 2).await.unwrap();

        let recs = vec![
            MemoryRecord {
                id: "1".into(),
                content: "Doc Hardware".into(),
                metadata: json_value!({"category": "hardware", "priority": "high"}),
                vectors: Some(vec![1.0, 0.0]),
            },
            MemoryRecord {
                id: "2".into(),
                content: "Doc Software".into(),
                metadata: json_value!({"category": "software"}),
                vectors: Some(vec![0.9, 0.1]),
            },
        ];
        store.add_documents(&manager, col, recs).await.unwrap();

        // Test 1: Recherche globale
        let res_all = store
            .search_similarity(&manager, col, &[1.0, 0.0], 10, 0.0, None)
            .await
            .unwrap();
        assert_eq!(res_all.len(), 2);

        // Test 2: Recherche avec filtre Metadata
        let mut filter = UnorderedMap::new();
        filter.insert("category".into(), "hardware".into());

        let res_filter = store
            .search_similarity(&manager, col, &[1.0, 0.0], 10, 0.0, Some(filter))
            .await
            .unwrap();
        assert_eq!(res_filter.len(), 1);
        assert_eq!(res_filter[0].id, "1");

        // Validation DB
        let db_doc = manager.get_document(col, "1").await.unwrap().unwrap();
        assert_eq!(db_doc["content"], "Doc Hardware");
    }

    #[async_test]
    async fn test_persistence_integrity() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        AgentDbSandbox::mock_db(&manager).await.unwrap();
        let path = sandbox.domain_root.join("vectors");
        let col = "p";

        {
            let store = CandleLocalStore::new(&path, &Device::Cpu);

            // 🎯 FIX : Il faut impérativement initialiser la collection pour que JSON-DB lui assigne son schéma !
            store.init_collection(&manager, col, 2).await.unwrap();

            let rec = MemoryRecord {
                id: "P1".into(),
                content: "Persist".into(),
                metadata: json_value!({"status": "saved"}),
                vectors: Some(vec![1.0, 0.0]),
            };
            // 🎯 L'appel ajoute, hydrate le tenseur, et sauvegarde de lui-même !
            store.add_documents(&manager, col, vec![rec]).await.unwrap();
        }

        // Simule un redémarrage du programme
        let new_store = CandleLocalStore::new(&path, &Device::Cpu);

        // 🎯 Magie noire : l'appel à search_similarity charge silencieusement le modèle existant
        let res = new_store
            .search_similarity(&manager, col, &[1.0, 0.0], 1, 0.9, None)
            .await
            .unwrap();

        assert_eq!(res.len(), 1);
        assert_eq!(res[0].metadata["status"], "saved");
    }
}
