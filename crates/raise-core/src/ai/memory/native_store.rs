// FICHIER : src-tauri/src/ai/memory/native_store.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

use super::{MemoryRecord, VectorStore};

/// Store vectoriel local RAISE agissant comme un index "Deep Learning"
/// pour les collections de données gérées par JSON-DB.
pub struct NativeLocalStore {
    device: ComputeHardware,
    /// Gestion isolée par Collection (Map) pour garantir l'étanchéité des domaines
    state: AsyncRwLock<UnorderedMap<String, CollectionState>>,
}

#[derive(Default, Clone)]
struct CollectionState {
    /// Lien direct : Ligne de la matrice -> `_id` du document dans JSON-DB
    index_to_id: Vec<String>,
    vector_matrix: Option<NeuralTensor>,
}

impl NativeLocalStore {
    /// Initialise le store vectoriel avec le périphérique spécifié.
    pub fn new(_path: &Path, device: &ComputeHardware) -> Self {
        Self {
            device: device.clone(),
            state: AsyncRwLock::new(UnorderedMap::new()),
        }
    }

    /// 🎯 RÉSOLUTION DÉTERMINISTE : Les tenseurs mémoires sont rangés dans la partition "tensors" de la DB.
    async fn get_tensor_dir(manager: &CollectionsManager<'_>, col: &str) -> PathBuf {
        manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("tensors")
            .join(col)
    }

    /// 🎯 LAZY LOADING : Charge les tenseurs depuis le SSD vers le ComputeHardware uniquement sur demande.
    async fn ensure_loaded(&self, manager: &CollectionsManager<'_>, col: &str) -> RaiseResult<()> {
        {
            let state = self.state.read().await;
            if let Some(cs) = state.get(col) {
                if cs.vector_matrix.is_some() {
                    return Ok(());
                }
            }
        }
        let mut state = self.state.write().await;
        if let Some(cs) = state.get(col) {
            if cs.vector_matrix.is_some() {
                return Ok(());
            }
        }

        let col_dir = Self::get_tensor_dir(manager, col).await;
        let index_path = col_dir.join("index.json");
        let tensor_path = col_dir.join("vectors.safetensors");

        let mut index_to_id = Vec::new();
        let mut matrix = None;

        if fs::exists_async(&index_path).await && fs::exists_async(&tensor_path).await {
            // Fail-Fast contre la corruption
            index_to_id = match fs::read_json_async(&index_path).await {
                Ok(idx) => idx,
                Err(e) => {
                    // On alerte l'utilisateur via le système de notification métier
                    user_error!(
                        "ERR_VECTOR_INDEX_CORRUPTED",
                        json_value!({
                            "path": index_path.to_string_lossy(),
                            "error": e.to_string(),
                            "hint": "Le lien entre les Tenseurs et JSON-DB est brisé. Une réindexation est nécessaire."
                        })
                    );
                    // On Fail-Fast pour protéger la base de données de toute écriture destructive
                    return Err(e);
                }
            };

            if !index_to_id.is_empty() {
                match SafeTensorsIO::load(&tensor_path, &self.device) {
                    Ok(mut tensors) => matrix = tensors.remove("vectors"),
                    Err(e) => {
                        user_warn!(
                            "WRN_VECTOR_LOAD_FAILED",
                            json_value!({"path": tensor_path.to_string_lossy(), "error": e.to_string()})
                        );
                    }
                }
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

        let index_path = col_dir.join("index.json");
        fs::write_json_atomic_async(&index_path, &col_state.index_to_id).await?;

        if let Some(ref matrix) = col_state.vector_matrix {
            let tensor_path = col_dir.join("vectors.safetensors");
            let matrix_clone = matrix.clone();

            let join_handle = spawn_cpu_task(move || {
                let mut map = UnorderedMap::new();
                map.insert("vectors".to_string(), matrix_clone);
                SafeTensorsIO::save(&map, tensor_path)
            })
            .await;

            // 🎯 FIX : Annotation explicite pour lever l'ambiguïté d'inférence de type
            match join_handle {
                Err(e) => raise_error!("ERR_THREAD_PANIC_DURING_SAVE", error = e.to_string()),
                Ok(Err(e)) => {
                    raise_error!("ERR_AI_SAFE_TENSORS_SAVE_FAILED", error = e.to_string())
                }
                Ok(Ok(())) => Ok::<(), AppError>(()), // 🎯 Annotation explicite ici
            }?;
        }
        Ok(())
    }

    /// Flush global de la mémoire vers le disque.
    /// Synchronise toutes les collections actuellement chargées en VRAM/RAM vers leurs partitions Safetensors respectives.
    pub async fn save(&self, manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let state = self.state.read().await;

        // On itère sur toutes les collections en cache pour forcer la persistance
        for (col_name, col_state) in state.iter() {
            self.save_collection(manager, col_name, col_state).await?;
        }

        user_info!(
            "INF_VECTOR_STORE_SYNC_COMPLETED",
            json_value!({
                "space": manager.space,
                "db": manager.db,
                "collections_flushed": state.len()
            })
        );
        Ok(())
    }

    /// Découverte à froid (Warm-up).
    /// Scanne le répertoire des tenseurs pour identifier les collections existantes
    /// et préparer le Lazy Loading sans saturer la VRAM immédiatement.
    pub async fn load(&self, manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        // Résolution du chemin racine des tenseurs pour cette DB
        let tensors_root = manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("tensors");

        if !fs::exists_async(&tensors_root).await {
            return Ok(());
        }

        // 🎯 FIX : read_dir_async nécessite une variable mutable pour le flux asynchrone
        let mut entries = fs::read_dir_async(&tensors_root).await?;
        let mut state = self.state.write().await;

        // 🎯 FIX : Boucle asynchrone idiomatique Tokio
        while let Ok(Some(entry)) = entries.next_entry().await {
            // Lecture asynchrone des métadonnées du fichier/dossier
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_dir() {
                    let col_name = entry.file_name().to_string_lossy().to_string();

                    // On pré-initialise l'état pour le Lazy Loading (vector_matrix est None)
                    state
                        .entry(col_name)
                        .or_insert_with(CollectionState::default);
                }
            }
        }

        user_info!(
            "INF_VECTOR_STORE_DISCOVERY",
            json_value!({
                "space": manager.space,
                "collections_discovered": state.len()
            })
        );
        Ok(())
    }
}

#[async_interface]
impl VectorStore for NativeLocalStore {
    async fn init_collection(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        _size: u64,
    ) -> RaiseResult<()> {
        let app_config = AppConfig::get();
        let col_dir = Self::get_tensor_dir(manager, collection_name).await;
        fs::ensure_dir_async(&col_dir).await?;

        // Résolution du schéma via mount points système
        let schema_uri = format!(
            "db://{}/{}/schemas/v2/agents/memory/vector_store_record.schema.json",
            app_config.mount_points.system.domain, app_config.mount_points.system.db
        );

        let _ = manager
            .create_collection(collection_name, &schema_uri)
            .await;

        self.ensure_loaded(manager, collection_name).await?;
        Ok(())
    }

    /// Ajoute des documents de manière synchronisée entre le moteur tensoriel et JSON-DB.
    /// ⚠️ CONTRAT STRICT : Les vecteurs fournis DOIVENT être pré-normalisés (L2)
    /// par le moteur NLP. Le calcul de similarité par `matmul` (Dot Product) ne vaut
    /// 'Cosine Similarity' que sous cette condition de normalisation.  
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

            let doc = json_value!({
                "_id": id.clone(),
                "content": rec.content,
                "metadata": rec.metadata
            });
            manager.upsert_document(collection_name, doc).await?;

            if let Some(vec) = rec.vectors {
                valid_vectors.push(vec);
                new_ids.push(id);
            }
        }

        if valid_vectors.is_empty() {
            return Ok(());
        }

        let mut state = self.state.write().await;
        let col_state = state
            .entry(collection_name.to_string())
            .or_insert_with(CollectionState::default);

        let n_new = valid_vectors.len();
        let d = valid_vectors[0].len();
        let flat_new: Vec<f32> = valid_vectors.into_iter().flatten().collect();

        let new_tensor = match NeuralTensor::from_vec(flat_new, (n_new, d), &self.device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_VECTOR_CREATION_FAILED", error = e.to_string()),
        };

        col_state.vector_matrix = match &col_state.vector_matrix {
            Some(existing) => match NeuralTensor::cat(&[existing, &new_tensor], 0) {
                Ok(t) => Some(t),
                Err(e) => raise_error!("ERR_VECTOR_CONCAT_FAILED", error = e.to_string()),
            },
            None => Some(new_tensor),
        };
        col_state.index_to_id.extend(new_ids);

        self.save_collection(manager, collection_name, col_state)
            .await?;

        Ok(())
    }

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

        let q = match NeuralTensor::from_slice(query_vec, (1, query_vec.len()), &self.device) {
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

        let mut ranked: Vec<(f32, usize)> = scores
            .into_iter()
            .enumerate()
            .filter(|(_, score)| *score >= threshold)
            .map(|(i, s)| (s, i))
            .collect();

        ranked.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(FmtOrdering::Equal));

        let mut results = Vec::new();

        for (_, idx) in ranked {
            if results.len() >= limit as usize {
                break;
            }
            let id = &col_state.index_to_id[idx];

            if let Ok(Some(doc)) = manager.get_document(collection_name, id).await {
                let mut meta_match = true;
                if let Some(ref f_map) = filter {
                    let doc_meta = doc.get("metadata").and_then(|m| m.as_object());
                    for (k, v) in f_map {
                        let val_match = doc_meta
                            .and_then(|m| m.get(k))
                            .map(|val| {
                                val.as_str().is_some_and(|s| s == v)
                                    || val
                                        .as_i64()
                                        .is_some_and(|n| v.parse::<i64>().is_ok_and(|p| p == n))
                                    || val
                                        .as_bool()
                                        .is_some_and(|b| v.parse::<bool>().is_ok_and(|p| p == b))
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
                        vectors: None,
                    });
                }
            }
        }
        Ok(results)
    }

    /// Libération explicite de la VRAM
    async fn unload_collection(&self, collection_name: &str) -> RaiseResult<()> {
        let mut state = self.state.write().await;

        if state.remove(collection_name).is_some() {
            user_info!(
                "INF_VECTOR_COLLECTION_UNLOADED",
                json_value!({
                    "collection": collection_name,
                    "status": "VRAM_RELEASED",
                    "hint": "La matrice tensorielle a été retirée de la mémoire active pour préserver les ressources GPU."
                })
            );
        }

        Ok(())
    }
}

// =========================================================================
// TESTS
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_full_search_with_metadata_filter() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let store = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);
        let col = "tech_resilient";
        store.init_collection(&manager, col, 2).await?;

        let recs = vec![MemoryRecord {
            id: "1".into(),
            content: "HW".into(),
            metadata: json_value!({"category": "hardware"}),
            vectors: Some(vec![1.0, 0.0]),
        }];
        store.add_documents(&manager, col, recs).await?;

        let mut filter = UnorderedMap::new();
        filter.insert("category".into(), "hardware".into());

        let res = store
            .search_similarity(&manager, col, &[1.0, 0.0], 1, 0.0, Some(filter))
            .await?;
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].id, "1");
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_persistence_mount_point_integrity() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let col = "persistence_test";
        {
            let store = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);
            store.init_collection(&manager, col, 2).await?;
            let rec = MemoryRecord {
                id: "P1".into(),
                content: "Data".into(),
                metadata: json_value!({"status": "saved"}),
                vectors: Some(vec![1.0, 0.0]),
            };
            store.add_documents(&manager, col, vec![rec]).await?;
        }

        let new_store = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);
        let res = new_store
            .search_similarity(&manager, col, &[1.0, 0.0], 1, 0.9, None)
            .await?;
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].metadata["status"], "saved");
        Ok(())
    }

    ///   Validation du cycle de vie VRAM
    #[async_test]
    #[serial_test::serial] // Protège contre les OOM croisés avec d'autres tests
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_unload_collection_vram_protection() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let store = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);
        let col = "test_vram_lifecycle";

        // =====================================================================
        // PHASE 1 : Initialisation et Chargement (VRAM = ACTIVE)
        // =====================================================================
        store.init_collection(&manager, col, 2).await?;
        let rec = MemoryRecord {
            id: "vram_1".into(),
            content: "Data Matrix".into(),
            metadata: json_value!({"status": "loaded"}),
            vectors: Some(vec![1.0, 0.0]),
        };
        store.add_documents(&manager, col, vec![rec]).await?;

        // PREUVE 1 : La matrice est bien en mémoire
        {
            let state = store.state.read().await;
            assert!(
                state.contains_key(col),
                "FAIL: La collection DOIT être chargée en mémoire après un ajout."
            );
        }

        // =====================================================================
        // PHASE 2 : Déchargement (VRAM = PURGÉE)
        // =====================================================================
        store.unload_collection(col).await?;

        // PREUVE 2 : La matrice a été détruite de la mémoire
        {
            let state = store.state.read().await;
            assert!(
                !state.contains_key(col),
                "FATAL OOM RISK: La collection est toujours en VRAM après le unload_collection !"
            );
        }

        // =====================================================================
        // PHASE 3 : Résilience et Lazy-Loading (Le RAG attaque à nouveau)
        // =====================================================================
        // On effectue une recherche sur la collection déchargée.
        // Le store DOIT comprendre qu'il doit la recharger silencieusement sans crasher.
        let res = store
            .search_similarity(&manager, col, &[1.0, 0.0], 1, 0.5, None)
            .await?;

        // PREUVE 3 : Le rechargement automatique a fonctionné
        assert_eq!(
            res.len(),
            1,
            "FAIL: Le Lazy-Loading après un unload a échoué."
        );
        assert_eq!(res[0].content, "Data Matrix");

        // PREUVE 4 : La matrice est de retour en mémoire
        {
            let state = store.state.read().await;
            assert!(
                state.contains_key(col),
                "FAIL: La collection devrait être de retour en RAM après une recherche."
            );
        }

        Ok(())
    }
    // =========================================================================
    // 🎯 FIX ZÉRO DETTE : Tests du Cycle de Vie Global (Save / Load / Warm-up)
    // =========================================================================

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_vector_store_save_and_load_lifecycle() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let col_name = "test_lifecycle_sync";

        // =====================================================================
        // PHASE 1 : Création, Indexation et Sauvegarde (Simulation: App Active)
        // =====================================================================
        {
            let store1 = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);
            store1.init_collection(&manager, col_name, 2).await?;

            let rec = MemoryRecord {
                id: "doc_persist_1".into(),
                content: "Donnée critique à sauvegarder".into(),
                metadata: json_value!({"mission": "alpha"}),
                vectors: Some(vec![1.0, 0.5]),
            };
            store1.add_documents(&manager, col_name, vec![rec]).await?;

            // Flush explicite de la mémoire vers les Safetensors
            store1.save(&manager).await?;
        } // `store1` est détruit ici. La VRAM est purgée. C'est l'équivalent d'un redémarrage.

        // =====================================================================
        // PHASE 2 : Découverte à froid (Simulation: App Redémarre)
        // =====================================================================
        let store2 = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);

        // Preuve 1 : Le Store démarre totalement vide (Sécurité VRAM respectée)
        assert!(
            store2.state.read().await.is_empty(),
            "Le store devrait être vierge au démarrage."
        );

        // Découverte silencieuse des collections sur le disque
        store2.load(&manager).await?;

        // Preuve 2 : Le Warm-up a fonctionné, MAIS la VRAM est toujours protégée
        {
            let state = store2.state.read().await;
            assert!(
                state.contains_key(col_name),
                "FAIL: Le load() n'a pas découvert le dossier de la collection."
            );
            if let Some(col_state) = state.get(col_name) {
                assert!(
                    col_state.vector_matrix.is_none(),
                    "FATAL OOM RISK: Le load() a chargé les tenseurs en VRAM au lieu de faire du Lazy-Loading !"
                );
            }
        }

        // =====================================================================
        // PHASE 3 : Reprise d'activité (Lazy-Loading déclenché par une requête)
        // =====================================================================
        let res = store2
            .search_similarity(&manager, col_name, &[1.0, 0.5], 1, 0.9, None)
            .await?;

        // Preuve 3 : Les données ont bien été restaurées depuis le SSD
        assert_eq!(
            res.len(),
            1,
            "Le Lazy-Loading n'a pas réussi à restaurer les documents."
        );
        assert_eq!(res[0].content, "Donnée critique à sauvegarder");

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_vector_store_load_empty_state_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let store = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);

        // Tenter un load sur une base de données complètement vierge (pas de dossier `tensors`)
        let res = store.load(&manager).await;

        assert!(
            res.is_ok(),
            "FAIL: Le chargement d'une base vierge devrait réussir silencieusement (No-Op)."
        );
        assert!(
            store.state.read().await.is_empty(),
            "L'état devrait rester vide."
        );

        Ok(())
    }
}
