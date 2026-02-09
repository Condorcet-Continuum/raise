// FICHIER : src-tauri/src/json_db/indexes/manager.rs

use super::{btree, hash, text, IndexDefinition, IndexType};
use crate::json_db::storage::StorageEngine;
use crate::utils::{
    error::{anyhow, AnyResult, Context},
    fs::{self, Path, PathBuf},
    json::{self, Deserialize, Serialize, Value},
};

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

    pub async fn create_index(
        &mut self,
        collection: &str,
        field: &str,
        kind_str: &str,
    ) -> AnyResult<()> {
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

        add_index_definition(self.storage, &self.space, &self.db, collection, def.clone()).await?;
        self.rebuild_index(collection, &def).await?;
        Ok(())
    }

    pub async fn drop_index(&mut self, collection: &str, field: &str) -> AnyResult<()> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            return Err(anyhow!("Collection introuvable"));
        }

        let mut meta = self.load_meta(&meta_path).await?;
        if let Some(pos) = meta.indexes.iter().position(|i| i.name == field) {
            let removed = meta.indexes.remove(pos);
            fs::write(&meta_path, json::stringify_pretty(&meta)?).await?;

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
                fs::remove_file(&index_path).await?;
            }
        } else {
            return Err(anyhow!("Index introuvable: {}", field));
        }
        Ok(())
    }

    /// Vérifie l'existence d'un index (Async pour éviter de bloquer sur l'I/O)
    pub async fn has_index(&self, collection: &str, field: &str) -> bool {
        if let Ok(indexes) = self.load_indexes(collection).await {
            return indexes.iter().any(|i| i.name == field);
        }
        false
    }

    pub async fn search(
        &self,
        collection: &str,
        field: &str,
        value: &Value,
    ) -> AnyResult<Vec<String>> {
        // Chargement async des définitions d'index
        let indexes = self.load_indexes(collection).await?;

        let def = indexes
            .iter()
            .find(|i| i.name == field)
            .ok_or_else(|| anyhow!("Index introuvable sur le champ '{}'", field))?;

        let cfg = &self.storage.config;
        let s = &self.space;
        let d = &self.db;

        match def.index_type {
            IndexType::Hash => hash::search_hash_index(cfg, s, d, collection, def, value).await,
            IndexType::BTree => btree::search_btree_index(cfg, s, d, collection, def, value).await,
            IndexType::Text => {
                let query_str = value.as_str().unwrap_or("").to_string();
                text::search_text_index(cfg, s, d, collection, def, &query_str).await
            }
        }
    }

    async fn rebuild_index(&self, collection: &str, def: &IndexDefinition) -> AnyResult<()> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);
        let indexes_dir = col_path.join("_indexes");
        fs::create_dir_all(&indexes_dir).await?;

        let mut entries = fs::read_dir(&col_path).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let filename = path.file_name().unwrap().to_str().unwrap();
                if filename.starts_with('_') {
                    continue;
                }

                let content = fs::read_to_string(&path).await?;
                if let Ok(doc) = json::parse::<Value>(&content) {
                    let doc_id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if !doc_id.is_empty() {
                        self.dispatch_update(collection, def, doc_id, None, Some(&doc))
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn index_document(&mut self, collection: &str, new_doc: &Value) -> AnyResult<()> {
        let doc_id = new_doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("ID manquant"))?;
        let indexes = self.load_indexes(collection).await?;

        if !indexes.is_empty() {
            let idx_dir = self
                .storage
                .config
                .db_collection_path(&self.space, &self.db, collection)
                .join("_indexes");
            fs::create_dir_all(idx_dir).await?;
        }

        for def in indexes {
            self.dispatch_update(collection, &def, doc_id, None, Some(new_doc))
                .await?;
        }
        Ok(())
    }

    pub async fn remove_document(&mut self, collection: &str, old_doc: &Value) -> AnyResult<()> {
        let doc_id = old_doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if doc_id.is_empty() {
            return Ok(());
        }

        for def in self.load_indexes(collection).await? {
            self.dispatch_update(collection, &def, doc_id, Some(old_doc), None)
                .await?;
        }
        Ok(())
    }

    async fn load_indexes(&self, collection: &str) -> AnyResult<Vec<IndexDefinition>> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            return Ok(Vec::new());
        }
        Ok(self.load_meta(&meta_path).await?.indexes)
    }

    fn get_meta_path(&self, collection: &str) -> PathBuf {
        self.storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json")
    }

    async fn load_meta(&self, path: &Path) -> AnyResult<CollectionMeta> {
        let content = fs::read_to_string(path).await?;
        Ok(json::parse(&content)?)
    }

    async fn dispatch_update(
        &self,
        col: &str,
        def: &IndexDefinition,
        id: &str,
        old: Option<&Value>,
        new: Option<&Value>,
    ) -> AnyResult<()> {
        let cfg = &self.storage.config;
        let s = &self.space;
        let d = &self.db;
        match def.index_type {
            IndexType::Hash => hash::update_hash_index(cfg, s, d, col, def, id, old, new).await,
            IndexType::BTree => btree::update_btree_index(cfg, s, d, col, def, id, old, new).await,
            IndexType::Text => text::update_text_index(cfg, s, d, col, def, id, old, new).await,
        }
        .with_context(|| format!("Erreur mise à jour index '{}'", def.name))
    }
}

pub async fn add_index_definition(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    def: IndexDefinition,
) -> AnyResult<()> {
    let meta_path = storage
        .config
        .db_collection_path(space, db, collection)
        .join("_meta.json");
    let mut meta: CollectionMeta = if meta_path.exists() {
        let content = fs::read_to_string(&meta_path).await?;
        json::parse(&content)?
    } else {
        CollectionMeta {
            schema: None,
            indexes: vec![],
        }
    };

    if !meta.indexes.iter().any(|i| i.name == def.name) {
        meta.indexes.push(def);
        fs::write(&meta_path, json::stringify_pretty(&meta)?).await?;
    }
    Ok(())
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::fs::tempdir;
    use crate::utils::json::json;

    #[tokio::test]
    async fn test_manager_lifecycle() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/users");
        fs::create_dir_all(&col_path).await.unwrap();

        mgr.create_index("users", "email", "hash").await.unwrap();
        assert!(col_path.join("_meta.json").exists());

        let doc = json!({ "id": "u1", "email": "a@a.com" });
        mgr.index_document("users", &doc).await.unwrap();

        let idx_path = col_path.join("_indexes/email.hash.idx");
        assert!(idx_path.exists());

        mgr.drop_index("users", "email").await.unwrap();
        assert!(!idx_path.exists());
    }

    #[tokio::test]
    async fn test_manager_search_flow() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/products");
        fs::create_dir_all(&col_path).await.unwrap();

        mgr.create_index("products", "category", "hash")
            .await
            .unwrap();

        let p1 = json!({ "id": "p1", "category": "book" });
        let p2 = json!({ "id": "p2", "category": "food" });

        mgr.index_document("products", &p1).await.unwrap();
        mgr.index_document("products", &p2).await.unwrap();

        // Correction : Appel async ici aussi
        assert!(mgr.has_index("products", "category").await);

        let results = mgr
            .search("products", "category", &json!("book"))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "p1");
    }
}
