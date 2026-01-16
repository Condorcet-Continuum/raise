// FICHIER : src-tauri/src/json_db/indexes/manager.rs

use super::{btree, hash, text, IndexDefinition, IndexType};
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize)]
struct CollectionMeta {
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub indexes: Vec<IndexDefinition>,
}

pub struct IndexManager<'a> {
    storage: &'a StorageEngine,
    space: String,
    db: String,
}

impl<'a> IndexManager<'a> {
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            storage,
            space: space.to_string(),
            db: db.to_string(),
        }
    }

    pub fn create_index(&mut self, collection: &str, field: &str, kind_str: &str) -> Result<()> {
        let kind = match kind_str.to_lowercase().as_str() {
            "hash" => IndexType::Hash,
            "btree" => IndexType::BTree,
            "text" => IndexType::Text,
            _ => return Err(anyhow!("Type d'index inconnu: {}", kind_str)),
        };

        let field_path = if field.starts_with('/') {
            field.to_string()
        } else {
            format!("/{}", field)
        };

        let def = IndexDefinition {
            name: field.to_string(),
            field_path,
            index_type: kind,
            unique: false,
        };

        add_index_definition(self.storage, &self.space, &self.db, collection, def.clone())?;
        self.rebuild_index(collection, &def)?;
        Ok(())
    }

    pub fn drop_index(&mut self, collection: &str, field: &str) -> Result<()> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            return Err(anyhow!("Collection introuvable"));
        }

        let mut meta = self.load_meta(&meta_path)?;
        if let Some(pos) = meta.indexes.iter().position(|i| i.name == field) {
            let removed = meta.indexes.remove(pos);
            fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

            let index_filename = match removed.index_type {
                IndexType::Hash => format!("{}.hash.idx", removed.name),
                IndexType::BTree => format!("{}.btree.idx", removed.name),
                IndexType::Text => format!("{}.text.idx", removed.name),
            };

            let index_path = self
                .storage
                .config
                .db_collection_path(&self.space, &self.db, collection)
                .join("_indexes")
                .join(index_filename);

            if index_path.exists() {
                fs::remove_file(index_path)?;
            }
        } else {
            return Err(anyhow!("Index introuvable: {}", field));
        }
        Ok(())
    }

    // --- RECHERCHE (NOUVEAU) ---

    pub fn has_index(&self, collection: &str, field: &str) -> bool {
        if let Ok(indexes) = self.load_indexes(collection) {
            return indexes.iter().any(|i| i.name == field);
        }
        false
    }

    pub fn search(&self, collection: &str, field: &str, value: &Value) -> Result<Vec<String>> {
        let indexes = self.load_indexes(collection)?;
        let def = indexes
            .iter()
            .find(|i| i.name == field)
            .ok_or_else(|| anyhow!("Index introuvable sur le champ '{}'", field))?;

        let cfg = &self.storage.config;
        let s = &self.space;
        let d = &self.db;

        match def.index_type {
            IndexType::Hash => hash::search_hash_index(cfg, s, d, collection, def, value),
            IndexType::BTree => btree::search_btree_index(cfg, s, d, collection, def, value),
            IndexType::Text => {
                let query_str = value.as_str().unwrap_or("").to_string();
                text::search_text_index(cfg, s, d, collection, def, &query_str)
            }
        }
    }

    // --- MAINTENANCE ---

    fn rebuild_index(&self, collection: &str, def: &IndexDefinition) -> Result<()> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);
        let indexes_dir = col_path.join("_indexes");
        fs::create_dir_all(&indexes_dir)?;

        for entry in fs::read_dir(&col_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let filename = path.file_name().unwrap().to_str().unwrap();
                if filename.starts_with('_') {
                    continue;
                }

                let content = fs::read_to_string(&path)?;
                if let Ok(doc) = serde_json::from_str::<Value>(&content) {
                    let doc_id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if !doc_id.is_empty() {
                        self.dispatch_update(collection, def, doc_id, None, Some(&doc))?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn index_document(&mut self, collection: &str, new_doc: &Value) -> Result<()> {
        let doc_id = new_doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or(anyhow!("ID manquant"))?;
        let indexes = self.load_indexes(collection)?;

        if !indexes.is_empty() {
            let idx_dir = self
                .storage
                .config
                .db_collection_path(&self.space, &self.db, collection)
                .join("_indexes");
            fs::create_dir_all(idx_dir)?;
        }

        for def in indexes {
            self.dispatch_update(collection, &def, doc_id, None, Some(new_doc))?;
        }
        Ok(())
    }

    pub fn remove_document(&mut self, collection: &str, old_doc: &Value) -> Result<()> {
        let doc_id = old_doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if doc_id.is_empty() {
            return Ok(());
        }

        for def in self.load_indexes(collection)? {
            self.dispatch_update(collection, &def, doc_id, Some(old_doc), None)?;
        }
        Ok(())
    }

    fn load_indexes(&self, collection: &str) -> Result<Vec<IndexDefinition>> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            return Ok(Vec::new());
        }
        Ok(self.load_meta(&meta_path)?.indexes)
    }

    fn get_meta_path(&self, collection: &str) -> PathBuf {
        self.storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json")
    }

    fn load_meta(&self, path: &PathBuf) -> Result<CollectionMeta> {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    }

    fn dispatch_update(
        &self,
        col: &str,
        def: &IndexDefinition,
        id: &str,
        old: Option<&Value>,
        new: Option<&Value>,
    ) -> Result<()> {
        let cfg = &self.storage.config;
        let s = &self.space;
        let d = &self.db;
        match def.index_type {
            IndexType::Hash => hash::update_hash_index(cfg, s, d, col, def, id, old, new),
            IndexType::BTree => btree::update_btree_index(cfg, s, d, col, def, id, old, new),
            IndexType::Text => text::update_text_index(cfg, s, d, col, def, id, old, new),
        }
        .with_context(|| format!("Erreur mise à jour index '{}'", def.name))
    }
}

pub fn add_index_definition(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    def: IndexDefinition,
) -> Result<()> {
    let meta_path = storage
        .config
        .db_collection_path(space, db, collection)
        .join("_meta.json");
    let mut meta: CollectionMeta = if meta_path.exists() {
        serde_json::from_str(&fs::read_to_string(&meta_path)?)?
    } else {
        CollectionMeta {
            schema: None,
            indexes: vec![],
        }
    };

    if !meta.indexes.iter().any(|i| i.name == def.name) {
        meta.indexes.push(def);
        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_manager_lifecycle() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let mut mgr = IndexManager::new(&storage, "s", "d");

        // Setup collection
        let col_path = dir.path().join("s/d/collections/users");
        fs::create_dir_all(&col_path).unwrap();

        // 1. Create Index
        mgr.create_index("users", "email", "hash").unwrap();
        assert!(col_path.join("_meta.json").exists());

        // 2. Index Document
        let doc = json!({ "id": "u1", "email": "a@a.com" });
        mgr.index_document("users", &doc).unwrap();

        // 3. Verify Index File
        let idx_path = col_path.join("_indexes/email.hash.idx");
        assert!(idx_path.exists());

        // 4. Drop Index
        mgr.drop_index("users", "email").unwrap();
        assert!(!idx_path.exists());
    }

    #[test]
    fn test_manager_search_flow() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/products");
        fs::create_dir_all(&col_path).unwrap();

        mgr.create_index("products", "category", "hash").unwrap();

        let p1 = json!({ "id": "p1", "category": "book" });
        let p2 = json!({ "id": "p2", "category": "food" });

        mgr.index_document("products", &p1).unwrap();
        mgr.index_document("products", &p2).unwrap();

        // Check has_index
        assert!(mgr.has_index("products", "category"));
        assert!(!mgr.has_index("products", "price"));

        // Search
        let results = mgr.search("products", "category", &json!("book")).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "p1");
    }
}
