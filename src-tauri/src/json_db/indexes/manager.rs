// FICHIER : src-tauri/src/json_db/indexes/manager.rs

use super::{btree, hash, text, IndexDefinition, IndexType};
use crate::json_db::storage::StorageEngine;

use crate::utils::io::{self, Path, PathBuf};
use crate::utils::json;
use crate::utils::prelude::*;

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
    ) -> RaiseResult<()> {
        let kind = match kind_str.to_lowercase().as_str() {
            "hash" => IndexType::Hash,
            "btree" => IndexType::BTree,
            "text" => IndexType::Text,
            _ => raise_error!(
                "ERR_DB_INDEX_TYPE_UNKNOWN",
                error = format!("Le type d'index '{}' n'est pas supporté.", kind_str),
                context = json!({
                    "attempted_type": kind_str,
                    "supported_types": ["hash", "btree", "text"],
                    "action": "parse_index_definition"
                })
            ),
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

    pub async fn drop_index(&mut self, collection: &str, field: &str) -> RaiseResult<()> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            raise_error!(
                "ERR_DB_COLLECTION_NOT_FOUND",
                error = "La collection spécifiée est introuvable sur le disque.",
                context = json!({
                    "meta_path": meta_path.to_string_lossy(),
                    "action": "load_collection_metadata",
                })
            );
        }

        let mut meta = self.load_meta(&meta_path).await?;
        if let Some(pos) = meta.indexes.iter().position(|i| i.name == field) {
            let removed = meta.indexes.remove(pos);
            io::write(&meta_path, json::stringify_pretty(&meta)?).await?;

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
                io::remove_file(&index_path).await?;
            }
        } else {
            raise_error!(
                "ERR_DB_INDEX_NOT_FOUND",
                error = format!("L'index associé au champ '{}' est introuvable.", field),
                context = json!({
                    "target_field": field,
                    "action": "index_resolution"
                })
            );
        }
        Ok(())
    }

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
    ) -> RaiseResult<Vec<String>> {
        let indexes = self.load_indexes(collection).await?;

        let Some(def) = indexes.iter().find(|i| i.name == field) else {
            raise_error!(
                "ERR_DB_INDEX_DEFINITION_NOT_FOUND",
                error = format!("Définition d'index introuvable pour le champ '{}'", field),
                context = json!({
                    "requested_field": field,
                    "available_indexes": indexes.iter().map(|i| &i.name).collect::<Vec<_>>()
                })
            );
        };

        let storage = self.storage;
        let s = &self.space;
        let d = &self.db;

        // ✅ Délégation avec le StorageEngine
        match def.index_type {
            IndexType::Hash => hash::search_hash_index(storage, s, d, collection, def, value).await,
            IndexType::BTree => {
                btree::search_btree_index(storage, s, d, collection, def, value).await
            }
            IndexType::Text => {
                let query_str = value.as_str().unwrap_or("").to_string();
                text::search_text_index(storage, s, d, collection, def, &query_str).await
            }
        }
    }

    async fn rebuild_index(&self, collection: &str, def: &IndexDefinition) -> RaiseResult<()> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);
        let indexes_dir = col_path.join("_indexes");
        io::create_dir_all(&indexes_dir).await?;

        // ✅ CORRECTION MAJEURE : On utilise list_document_ids et le StorageEngine
        // Au lieu de lire le disque physiquement, on laisse le cache LRU faire son travail !
        let ids = crate::json_db::collections::collection::list_document_ids(
            &self.storage.config,
            &self.space,
            &self.db,
            collection,
        )
        .await?;

        for id in ids {
            if let Ok(Some(doc)) = self
                .storage
                .read_document(&self.space, &self.db, collection, &id)
                .await
            {
                self.dispatch_update(collection, def, &id, None, Some(&doc))
                    .await?;
            }
        }
        Ok(())
    }

    pub async fn index_document(&mut self, collection: &str, new_doc: &Value) -> RaiseResult<()> {
        let Some(doc_id) = new_doc.get("_id").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_DB_DOCUMENT_ID_MISSING",
                error = "Impossible d'indexer le document : attribut '_id' manquant ou invalide.",
                context = json!({
                    "collection": collection,
                    "action": "index_document",
                    "hint": "Assurez-vous que le document est passé par le SchemaValidator (qui génère l'_id) avant d'atteindre le moteur d'indexation."
                })
            );
        };
        let indexes = self.load_indexes(collection).await?;

        if !indexes.is_empty() {
            let idx_dir = self
                .storage
                .config
                .db_collection_path(&self.space, &self.db, collection)
                .join("_indexes");
            io::create_dir_all(idx_dir).await?;
        }

        for def in indexes {
            self.dispatch_update(collection, &def, doc_id, None, Some(new_doc))
                .await?;
        }
        Ok(())
    }

    pub async fn remove_document(&mut self, collection: &str, old_doc: &Value) -> RaiseResult<()> {
        let doc_id = old_doc.get("_id").and_then(|v| v.as_str()).unwrap_or("");
        if doc_id.is_empty() {
            return Ok(());
        }

        for def in self.load_indexes(collection).await? {
            self.dispatch_update(collection, &def, doc_id, Some(old_doc), None)
                .await?;
        }
        Ok(())
    }
    pub async fn list_indexes(
        &self,
        collection: &str,
        field_filter: Option<&str>,
    ) -> RaiseResult<Vec<IndexDefinition>> {
        let all_indexes = self.load_indexes(collection).await?;

        if let Some(field) = field_filter {
            // On filtre pour ne garder que l'index qui correspond au champ demandé
            let filtered = all_indexes
                .into_iter()
                .filter(|idx| idx.name == field)
                .collect();
            Ok(filtered)
        } else {
            // Pas de filtre, on retourne tout
            Ok(all_indexes)
        }
    }
    async fn load_indexes(&self, collection: &str) -> RaiseResult<Vec<IndexDefinition>> {
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

    async fn load_meta(&self, path: &Path) -> RaiseResult<CollectionMeta> {
        let content = io::read_to_string(path).await?;
        json::parse(&content)
    }

    async fn dispatch_update(
        &self,
        col: &str,
        def: &IndexDefinition,
        id: &str,
        old: Option<&Value>,
        new: Option<&Value>,
    ) -> RaiseResult<()> {
        let storage = self.storage; // ✅ On passe le StorageEngine
        let s = &self.space;
        let d = &self.db;

        let result = match def.index_type {
            IndexType::Hash => hash::update_hash_index(storage, s, d, col, def, id, old, new).await,
            IndexType::BTree => {
                btree::update_btree_index(storage, s, d, col, def, id, old, new).await
            }
            IndexType::Text => text::update_text_index(storage, s, d, col, def, id, old, new).await,
        };

        if let Err(e) = result {
            raise_error!(
                "ERR_DB_INDEX_UPDATE_FAIL",
                error = e,
                context = json!({ "index_name": def.name })
            );
        }
        Ok(())
    }
}

pub async fn add_index_definition(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    def: IndexDefinition,
) -> RaiseResult<()> {
    let meta_path = storage
        .config
        .db_collection_path(space, db, collection)
        .join("_meta.json");
    let mut meta: CollectionMeta = if meta_path.exists() {
        let content = io::read_to_string(&meta_path).await?;
        json::parse(&content)?
    } else {
        CollectionMeta {
            schema: None,
            indexes: vec![],
        }
    };

    if !meta.indexes.iter().any(|i| i.name == def.name) {
        meta.indexes.push(def);
        io::write(&meta_path, json::stringify_pretty(&meta)?).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;
    use crate::utils::io::tempdir;
    use crate::utils::json::json;

    #[tokio::test]
    async fn test_manager_lifecycle() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/users");
        io::create_dir_all(&col_path).await.unwrap();

        mgr.create_index("users", "email", "hash").await.unwrap();
        assert!(col_path.join("_meta.json").exists());

        // ✅ CORRECTION : Utilisation de "_id"
        let doc = json!({ "_id": "u1", "email": "a@a.com" });
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
        io::create_dir_all(&col_path).await.unwrap();

        mgr.create_index("products", "category", "hash")
            .await
            .unwrap();

        // ✅ CORRECTION : Utilisation de "_id"
        let p1 = json!({ "_id": "p1", "category": "book" });
        let p2 = json!({ "_id": "p2", "category": "food" });

        mgr.index_document("products", &p1).await.unwrap();
        mgr.index_document("products", &p2).await.unwrap();

        assert!(mgr.has_index("products", "category").await);

        let results = mgr
            .search("products", "category", &json!("book"))
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "p1");
    }

    #[tokio::test]
    async fn test_list_indexes_filtering() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/users");
        io::create_dir_all(&col_path).await.unwrap();

        mgr.create_index("users", "email", "hash").await.unwrap();
        mgr.create_index("users", "age", "btree").await.unwrap();

        let all_indexes = mgr.list_indexes("users", None).await.unwrap();
        assert_eq!(all_indexes.len(), 2, "Il devrait y avoir 2 index au total");

        let email_index = mgr.list_indexes("users", Some("email")).await.unwrap();
        assert_eq!(
            email_index.len(),
            1,
            "Le filtre devrait retourner 1 seul index"
        );
        assert_eq!(email_index[0].name, "email");

        let age_index = mgr.list_indexes("users", Some("age")).await.unwrap();
        assert_eq!(age_index.len(), 1);
        assert_eq!(age_index[0].name, "age");

        let unknown_index = mgr.list_indexes("users", Some("not_found")).await.unwrap();
        assert!(
            unknown_index.is_empty(),
            "La liste devrait être vide pour un index inexistant"
        );
    }
}
