// FICHIER : src-tauri/src/json_db/storage/mod.rs

pub mod cache;
pub mod compression;
pub mod file_storage;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::PathBuf;

// --- CONFIGURATION ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonDbConfig {
    pub data_root: PathBuf,
}

impl JsonDbConfig {
    pub fn new(data_root: PathBuf) -> Self {
        Self { data_root }
    }

    pub fn from(path_str: String) -> Result<Self, String> {
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
        // Centralisation absolue dans _system/schemas
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
            cache: cache::Cache::new(1000, None),
        }
    }

    pub fn write_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
        doc: &Value,
    ) -> Result<()> {
        // 1. Écriture disque (Persistance)
        file_storage::write_document(&self.config, space, db, collection, id, doc)?;

        // 2. Mise à jour cache (Performance)
        let cache_key = format!("{}/{}/{}/{}", space, db, collection, id);
        self.cache.put(cache_key, doc.clone());

        Ok(())
    }

    pub fn read_document(
        &self,
        space: &str,
        db: &str,
        collection: &str,
        id: &str,
    ) -> Result<Option<Value>> {
        let cache_key = format!("{}/{}/{}/{}", space, db, collection, id);

        // 1. Vérification cache
        if let Some(doc) = self.cache.get(&cache_key) {
            return Ok(Some(doc));
        }

        // 2. Lecture disque
        let doc_opt = file_storage::read_document(&self.config, space, db, collection, id)?;

        // 3. Peuplement cache
        if let Some(doc) = &doc_opt {
            self.cache.put(cache_key, doc.clone());
        }

        Ok(doc_opt)
    }

    pub fn delete_document(&self, space: &str, db: &str, collection: &str, id: &str) -> Result<()> {
        file_storage::delete_document(&self.config, space, db, collection, id)?;
        let cache_key = format!("{}/{}/{}/{}", space, db, collection, id);
        self.cache.remove(&cache_key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_storage_engine_cache_hit() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let engine = StorageEngine::new(config);

        let doc = json!({"val": 42});

        // Écriture
        engine.write_document("s", "d", "c", "1", &doc).unwrap();

        // Vérifions que c'est dans le cache
        assert!(engine.cache.get(&"s/d/c/1".to_string()).is_some());

        // Lecture (doit venir du cache - on pourrait supprimer le fichier pour le prouver)
        let read = engine.read_document("s", "d", "c", "1").unwrap().unwrap();
        assert_eq!(read["val"], 42);
    }
}
