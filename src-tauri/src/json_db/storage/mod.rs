// FICHIER : src-tauri/src/json_db/storage/mod.rs

pub mod cache;
pub mod file_storage;

use crate::utils::data::{Deserialize, Serialize, Value};
use crate::utils::error::RaiseResult;
use crate::utils::io::PathBuf;

// --- CONFIGURATION ---

#[derive(Debug, Clone, Serialize, Deserialize)]
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

    pub fn db_schemas_root(&self, space: &str, _db: &str) -> PathBuf {
        self.db_root(space, "_system").join("schemas")
    }
}

// --- MOTEUR DE STOCKAGE ---

#[derive(Debug, Clone)]
pub struct StorageEngine {
    pub config: JsonDbConfig,
    pub cache: cache::Cache<String, Value>,
}

impl StorageEngine {
    pub fn new(config: JsonDbConfig) -> Self {
        Self {
            config,
            // Utilisation d'une capacité de 1000 avec la nouvelle logique LRU
            cache: cache::Cache::new(1000, None),
        }
    }

    /// Écrit un document de manière asynchrone (Disque + Cache)
    pub async fn write_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
        doc: &Value,
    ) -> RaiseResult<()> {
        // 1. Écriture disque atomique et asynchrone
        file_storage::write_document(&self.config, space, db, collection, id, doc).await?;

        // 2. Mise à jour du cache LRU (opération synchrone en RAM)
        let cache_key = format!("{}/{}/{}/{}", space, db, collection, id);
        self.cache.put(cache_key, doc.clone());

        Ok(())
    }

    /// Lit un document (Cache Hit d'abord, sinon Disque Async)
    pub async fn read_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
    ) -> RaiseResult<Option<Value>> {
        let cache_key = format!("{}/{}/{}/{}", space, db, collection, id);

        // 1. Vérification du cache
        if let Some(doc) = self.cache.get(&cache_key) {
            return Ok(Some(doc));
        }

        // 2. Lecture disque asynchrone
        let doc_opt = file_storage::read_document(&self.config, space, db, collection, id).await?;

        // 3. Mise en cache si trouvé
        if let Some(doc) = &doc_opt {
            self.cache.put(cache_key, doc.clone());
        }

        Ok(doc_opt)
    }

    /// Supprime un document (Disque Async + Cache)
    pub async fn delete_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
    ) -> RaiseResult<()> {
        // Suppression disque
        file_storage::delete_document(&self.config, space, db, collection, id).await?;

        // Suppression cache
        let cache_key = format!("{}/{}/{}/{}", space, db, collection, id);
        self.cache.remove(&cache_key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{io::tempdir, json::json};

    #[tokio::test]
    async fn test_storage_engine_cache_hit() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let engine = StorageEngine::new(config);

        let doc = json!({"val": 42});

        // Test écriture
        engine
            .write_document("s", "d", "c", "1", &doc)
            .await
            .unwrap();

        // Le cache doit contenir la valeur
        assert!(engine.cache.get(&"s/d/c/1".to_string()).is_some());

        // Lecture (doit retourner la valeur, idéalement depuis le cache)
        let read = engine
            .read_document("s", "d", "c", "1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read["val"], 42);
    }
}
