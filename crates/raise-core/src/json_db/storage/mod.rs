// FICHIER : src-tauri/src/json_db/storage/mod.rs
use crate::utils::prelude::*;

pub mod cache;
pub mod file_storage;

// --- CONFIGURATION ---

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct JsonDbConfig {
    pub data_root: PathBuf,
}

impl JsonDbConfig {
    pub fn new(data_root: PathBuf) -> Self {
        Self { data_root }
    }

    pub fn from(path_str: String) -> RaiseResult<Self> {
        Ok(Self {
            data_root: PathBuf::from(path_str),
        })
    }

    pub fn db_root(&self, space: &str, db: &str) -> PathBuf {
        self.data_root.join(space).join(db)
    }

    pub fn db_collection_path(&self, space: &str, db: &str, collection: &str) -> PathBuf {
        self.db_root(space, db).join("collections").join(collection)
    }

    pub fn db_schemas_root(&self, space: &str, db: &str) -> PathBuf {
        self.db_root(space, db).join("schemas")
    }
}

// --- MOTEUR DE STOCKAGE ---

#[derive(Debug, Clone)]
pub struct StorageEngine {
    pub config: JsonDbConfig,
    // ✅ OPTIMISATION : Une clé structurée plutôt qu'une chaîne de caractères formatée !
    pub cache: cache::Cache<(String, String, String, String), JsonValue>,
    //  Registre de verrous exclusifs pour les index système (Anti Race-Condition)
    pub index_locks: SharedRef<SyncRwLock<UnorderedMap<String, SharedRef<AsyncMutex<()>>>>>,
}

impl StorageEngine {
    pub fn new(config: JsonDbConfig) -> RaiseResult<Self> {
        // Initialisation du cache (1000 entrées par défaut)
        let cache = cache::Cache::new(1000, None)?;

        Ok(Self {
            config,
            cache,
            index_locks: SharedRef::new(SyncRwLock::new(UnorderedMap::new())),
        })
    }

    /// Réclame un verrou exclusif asynchrone pour un index système.
    /// Gère l'empoisonnement du verrou synchrone interne.
    pub fn get_index_lock(&self, space: &str, db: &str) -> RaiseResult<SharedRef<AsyncMutex<()>>> {
        let key = format!("{}/{}", space, db);

        // Verrouillage synchrone de la map de verrous
        let mut map = match self.index_locks.write() {
            Ok(guard) => guard,
            Err(e) => {
                raise_error!(
                    "ERR_STORAGE_LOCK_REGISTRY_POISONED",
                    error = e.to_string(),
                    context = json_value!({ "space": space, "db": db })
                );
            }
        };

        let lock = map
            .entry(key)
            .or_insert_with(|| SharedRef::new(AsyncMutex::new(())))
            .clone();

        Ok(lock)
    }

    /// Lit un document en cherchant d'abord dans le cache LRU
    pub async fn read_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
    ) -> RaiseResult<Option<JsonValue>> {
        let cache_key = (
            space.to_string(),
            db.to_string(),
            collection.to_string(),
            id.to_string(),
        );

        // 1. Recherche en Cache (Sync & Immédiat)
        // .get() renvoie maintenant un RaiseResult<Option<V>>
        match self.cache.get(&cache_key) {
            Ok(Some(doc)) => return Ok(Some(doc)),
            Ok(None) => (),          // Cache Miss
            Err(e) => return Err(e), // Erreur critique (Verrou)
        }

        // 2. Cache Miss : Lecture disque
        let doc_opt =
            match file_storage::read_document(&self.config, space, db, collection, id).await {
                Ok(d) => d,
                Err(e) => return Err(e),
            };

        // 3. Hydratation du cache
        if let Some(doc) = &doc_opt {
            match self.cache.put(cache_key, doc.clone()) {
                Ok(_) => (),
                Err(e) => return Err(e),
            }
        }

        Ok(doc_opt)
    }

    /// Écrit un document sur le disque et met à jour le cache
    pub async fn write_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
        doc: &JsonValue,
    ) -> RaiseResult<()> {
        if id.is_empty() {
            raise_error!(
                "ERR_DB_WRITE_EMPTY_ID",
                context = json_value!({ "collection": collection })
            );
        }

        file_storage::write_document(&self.config, space, db, collection, id, doc).await?;

        let cache_key = (
            space.to_string(),
            db.to_string(),
            collection.to_string(),
            id.to_string(),
        );

        // Write-through en mémoire (.await)
        self.cache.put(cache_key, doc.clone())?;
        Ok(())
    }

    /// Supprime un document (Disque Async + Cache)
    pub async fn delete_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
    ) -> RaiseResult<()> {
        file_storage::delete_document(&self.config, space, db, collection, id).await?;

        let cache_key = (
            space.to_string(),
            db.to_string(),
            collection.to_string(),
            id.to_string(),
        );

        self.cache.remove(&cache_key)?;
        Ok(())
    }

    pub async fn auto_recover_all(&self) -> RaiseResult<usize> {
        let mut total_recovered = 0;
        let root = &self.config.data_root;

        if !fs::exists_async(root).await {
            return Ok(0);
        }

        // 1. Lister les "Spaces" (Dossiers à la racine)
        let mut spaces = fs::read_dir_async(root).await?;
        while let Some(space_entry) = spaces.next_entry().await? {
            if space_entry.file_type().await?.is_dir() {
                let space_name = space_entry.file_name().to_string_lossy().to_string();

                // 2. Lister les "Databases" dans chaque Space
                let mut dbs = fs::read_dir_async(&space_entry.path()).await?;
                while let Some(db_entry) = dbs.next_entry().await? {
                    if db_entry.file_type().await?.is_dir() {
                        let db_name = db_entry.file_name().to_string_lossy().to_string();

                        // 3. Lancer la récupération WAL pour ce couple Space/DB
                        let count =
                            crate::json_db::transactions::wal::recover_pending_transactions(
                                &self.config,
                                &space_name,
                                &db_name,
                                self,
                            )
                            .await?;

                        total_recovered += count;
                    }
                }
            }
        }

        if total_recovered > 0 {
            user_info!(
                "RECOVERY_COMPLETE",
                json_value!({ "recovered_transactions": total_recovered })
            );
        }

        Ok(total_recovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_storage_engine_cache_hit() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // 🎯 FIX : Extraction de l'instance du Result avant utilisation
        let engine = match StorageEngine::new(config) {
            Ok(e) => e,
            Err(e) => return Err(e),
        };

        let doc = json_value!({"val": 42});

        // 🎯 FIX : Utilisation de '?' autorisée sur RaiseResult<()>
        engine.write_document("s", "d", "c", "1", &doc).await?;

        let key = (
            "s".to_string(),
            "d".to_string(),
            "c".to_string(),
            "1".to_string(),
        );

        // Vérification du cache (get renvoie RaiseResult<Option>)
        let cached = match engine.cache.get(&key) {
            Ok(opt) => opt,
            Err(e) => return Err(e),
        };
        assert!(cached.is_some());

        let read = engine.read_document("s", "d", "c", "1").await?;
        assert_eq!(read, Some(doc));

        engine.delete_document("s", "d", "c", "1").await?;

        let cached_after = match engine.cache.get(&key) {
            Ok(opt) => opt,
            Err(e) => return Err(e),
        };
        assert!(cached_after.is_none());

        Ok(())
    }

    #[async_test]
    async fn test_index_lock_prevents_race_condition() -> RaiseResult<()> {
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Échec création dossier temporaire : {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // 🎯 FIX : Extraction et wrapping dans SharedRef (Arc)
        let engine_inner = match StorageEngine::new(config.clone()) {
            Ok(e) => e,
            Err(e) => return Err(e),
        };
        let engine = SharedRef::new(engine_inner);

        let space = "concurrent_space";
        let db = "concurrent_db";

        let sys_path = config.db_root(space, db).join("_system.json");

        // Préparation du dossier
        let parent = match sys_path.parent() {
            Some(p) => p,
            None => {
                // Ici raise_error! fait son 'return Err(...)' et sort de la fonction
                raise_error!(
                    "ERR_DB_INVALID_PATH",
                    error = "Impossible de trouver le dossier parent pour le fichier système.",
                    context = json_value!({ "path": sys_path.to_string_lossy() })
                );
            }
        };
        fs::ensure_dir_async(parent).await?;

        fs::write_json_atomic_async(&sys_path, &json_value!({"counter": 0})).await?;

        let mut handles = Vec::new();

        for _ in 0..50 {
            let engine_clone = engine.clone();
            let sys_path_clone = sys_path.clone();

            handles.push(spawn_async_task(async move {
                // 🎯 FIX : get_index_lock renvoie désormais RaiseResult
                let lock = match engine_clone.get_index_lock("concurrent_space", "concurrent_db") {
                    Ok(l) => l,
                    Err(_) => panic!("Échec acquisition lock registre"),
                };

                let _guard = lock.lock().await;

                let mut doc: JsonValue = fs::read_json_async(&sys_path_clone).await?;

                let current = doc["counter"].as_i64().expect("Type error");
                doc["counter"] = json_value!(current + 1);

                fs::write_json_atomic_async(&sys_path_clone, &doc).await?;

                Ok::<(), AppError>(())
            }));
        }

        for handle in handles {
            match handle.await {
                Ok(task_result) => task_result?,
                Err(e) => raise_error!("ERR_TASK_JOIN", error = e),
            }
        }

        let final_doc: JsonValue = fs::read_json_async(&sys_path).await?;
        assert_eq!(final_doc["counter"], 50);

        Ok(())
    }
}

#[cfg(test)]
mod recovery_tests {
    use super::*;
    use crate::json_db::transactions::{Operation, Transaction};

    #[async_test]
    async fn test_storage_engine_auto_recovery_scenario() -> RaiseResult<()> {
        let dir = tempdir().expect("Fail TempDir");
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config.clone())?;

        let space = "test_space";
        let db = "test_db";
        let col = "system_configs";

        // --- PHASE 1 : ÉTAT STABLE ---
        let original_doc = json_value!({ "_id": "cfg_1", "theme": "dark", "version": 1 });
        storage
            .write_document(space, db, col, "cfg_1", &original_doc)
            .await?;

        // --- PHASE 2 : SIMULATION CRASH PENDANT UPDATE ---
        // On crée une transaction WAL qui contient l'image de secours (Undo)
        let tx = Transaction {
            id: "tx_crash_sim".to_string(),
            operations: vec![Operation::Update {
                collection: col.to_string(),
                id: "cfg_1".to_string(),
                previous_document: Some(original_doc.clone()), // La bouée de sauvetage
                document: json_value!({ "theme": "corrupted_light" }),
            }],
        };

        // On écrit le WAL manuellement (comme si le manager l'avait fait avant de crasher)
        crate::json_db::transactions::wal::write_entry(&config, space, db, &tx).await?;

        // On simule l'écriture physique partielle/corrompue sur le disque
        let corrupted_doc =
            json_value!({ "_id": "cfg_1", "theme": "corrupted_light", "version": 1 });
        storage
            .write_document(space, db, col, "cfg_1", &corrupted_doc)
            .await?;

        // --- PHASE 3 : RÉSURRECTION ---
        // Le moteur redémarre et lance sa procédure de scan
        let recovered_count = storage.auto_recover_all().await?;

        // --- ASSERTIONS ---
        assert_eq!(recovered_count, 1, "Une transaction aurait dû être réparée");

        // La donnée sur le disque doit être revenue à son état initial (original_doc)
        let restored_doc = storage
            .read_document(space, db, col, "cfg_1")
            .await?
            .unwrap();
        assert_eq!(
            restored_doc["theme"], "dark",
            "Le rollback (Undo) a échoué !"
        );
        assert_eq!(restored_doc["version"], 1);

        // Le fichier WAL doit avoir été nettoyé
        let wal_dir = config.db_root(space, db).join("wal");
        let wal_file = wal_dir.join("tx_crash_sim.json");
        assert!(
            !fs::exists_async(&wal_file).await,
            "Le fichier WAL n'a pas été supprimé après recovery"
        );

        Ok(())
    }
}
