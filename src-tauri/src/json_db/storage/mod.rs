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
    pub fn new(config: JsonDbConfig) -> Self {
        Self {
            config,
            cache: cache::Cache::new(1000, None),
            index_locks: SharedRef::new(SyncRwLock::new(UnorderedMap::new())),
        }
    }
    pub fn get_index_lock(&self, space: &str, db: &str) -> SharedRef<AsyncMutex<()>> {
        let key = format!("{}/{}", space, db);

        // Verrou synchrone ultra-court juste pour lire/écrire dans la HashMap
        let mut map = self.index_locks.write().unwrap();

        map.entry(key)
            .or_insert_with(|| SharedRef::new(AsyncMutex::new(())))
            .clone()
    }
    /// Lit un document en cherchant d'abord dans le cache LRU
    pub async fn read_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
    ) -> RaiseResult<Option<JsonValue>> {
        // La création d'un tuple est plus rapide qu'un format! macro
        let cache_key = (
            space.to_string(),
            db.to_string(),
            collection.to_string(),
            id.to_string(),
        );

        // ✅ Point de suspension `.await`
        if let Some(doc) = self.cache.get(&cache_key).await {
            return Ok(Some(doc));
        }

        // Cache Miss : Lecture asynchrone sur disque
        let doc_opt = file_storage::read_document(&self.config, space, db, collection, id).await?;

        // Hydratation du cache si le document existe
        if let Some(doc) = &doc_opt {
            self.cache.put(cache_key, doc.clone()).await;
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
        file_storage::write_document(&self.config, space, db, collection, id, doc).await?;

        let cache_key = (
            space.to_string(),
            db.to_string(),
            collection.to_string(),
            id.to_string(),
        );

        // Write-through en mémoire (.await)
        self.cache.put(cache_key, doc.clone()).await;
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

        self.cache.remove(&cache_key).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_storage_engine_cache_hit() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let engine = StorageEngine::new(config);

        let doc = json_value!({"val": 42});

        engine
            .write_document("s", "d", "c", "1", &doc)
            .await
            .unwrap();

        // On utilise la nouvelle structure de clé
        let key = (
            "s".to_string(),
            "d".to_string(),
            "c".to_string(),
            "1".to_string(),
        );

        assert!(engine.cache.get(&key).await.is_some());

        let read = engine.read_document("s", "d", "c", "1").await.unwrap();
        assert_eq!(read, Some(doc));

        engine.delete_document("s", "d", "c", "1").await.unwrap();
        assert!(engine.cache.get(&key).await.is_none());
    }

    #[async_test]
    async fn test_index_lock_prevents_race_condition() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // On wrap l'engine dans un Arc/SharedRef pour pouvoir le cloner dans 50 threads
        let engine = SharedRef::new(StorageEngine::new(config.clone()));
        let space = "concurrent_space";
        let db = "concurrent_db";

        // 1. Initialisation d'un faux fichier d'index avec un compteur à 0
        let sys_path = config.db_root(space, db).join("_system.json");
        fs::ensure_dir_async(sys_path.parent().unwrap())
            .await
            .unwrap();
        fs::write_json_atomic_async(&sys_path, &json_value!({"counter": 0}))
            .await
            .unwrap();

        let mut handles = Vec::new();

        // 2. On lance 50 "transactions" en parallèle absolu !
        for _ in 0..50 {
            let engine_clone = engine.clone();
            let sys_path_clone = sys_path.clone();

            handles.push(spawn_async_task(async move {
                // 🎯 LA MAGIE EST ICI : On réclame le verrou exclusif pour cette base
                let lock = engine_clone.get_index_lock("concurrent_space", "concurrent_db");
                let _guard = lock.lock().await; // Attente passive asynchrone sans bloquer le CPU

                // --- DÉBUT DE LA ZONE CRITIQUE ---
                // a. Lecture
                let mut doc: JsonValue = fs::read_json_async(&sys_path_clone).await.unwrap();

                // b. Modification (On perd virtuellement un peu de temps pour exacerber le risque)
                let current = doc["counter"].as_i64().unwrap();
                doc["counter"] = json_value!(current + 1);

                // c. Écriture
                fs::write_json_atomic_async(&sys_path_clone, &doc)
                    .await
                    .unwrap();
                // --- FIN DE LA ZONE CRITIQUE ---

                // Le verrou est relâché automatiquement ici quand `_guard` sort de la portée (Drop)
            }));
        }

        // 3. On attend que nos 50 kamikazes aient terminé
        for handle in handles {
            let _ = handle.await;
        }

        // 4. L'HEURE DE VÉRITÉ
        let final_doc: JsonValue = fs::read_json_async(&sys_path).await.unwrap();

        assert_eq!(
            final_doc["counter"], 50,
            "💥 RACE CONDITION DÉTECTÉE ! Le verrou a failli, des mises à jour ont été écrasées."
        );
    }
}
