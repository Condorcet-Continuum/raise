// FICHIER : src-tauri/src/json_db/indexes/manager.rs

use super::{btree, hash, text, IndexDefinition, IndexType};
use crate::json_db::collections::collection;
use crate::json_db::storage::StorageEngine;

use crate::utils::prelude::*;

#[derive(Debug, Serializable, Deserializable)]
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
                context = json_value!({
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
        if !fs::exists_async(&meta_path).await {
            raise_error!(
                "ERR_DB_COLLECTION_NOT_FOUND",
                context = json_value!({ "coll": collection })
            );
        }

        let mut meta = self.load_meta(&meta_path).await?;
        if let Some(pos) = meta.indexes.iter().position(|i| i.name == field) {
            let removed = meta.indexes.remove(pos);

            match fs::write_json_atomic_async(&meta_path, &meta).await {
                Ok(_) => (),
                Err(e) => raise_error!("ERR_DB_META_SAVE_FAILED", error = e),
            };

            let index_path = crate::json_db::indexes::paths::index_path(
                &self.storage.config,
                &self.space,
                &self.db,
                collection,
                &removed.name,
                removed.index_type,
            );

            if fs::exists_async(&index_path).await {
                match fs::remove_file_async(&index_path).await {
                    Ok(_) => (),
                    Err(e) => user_warn!(
                        "WRN_FS_INDEX_DELETE_FAILED",
                        json_value!({"path": index_path, "error": e.to_string()})
                    ),
                }
            }
        } else {
            raise_error!(
                "ERR_DB_INDEX_NOT_FOUND",
                error = format!("L'index associé au champ '{}' est introuvable.", field),
                context = json_value!({
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
        value: &JsonValue,
    ) -> RaiseResult<Vec<String>> {
        let indexes = self.load_indexes(collection).await?;

        let Some(def) = indexes.iter().find(|i| i.name == field) else {
            raise_error!(
                "ERR_DB_INDEX_DEFINITION_NOT_FOUND",
                error = format!("Définition d'index introuvable pour le champ '{}'", field),
                context = json_value!({
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
        match fs::create_dir_all_async(&indexes_dir).await {
            Ok(_) => (),
            Err(e) => raise_error!("ERR_FS_INDEX_DIR_FAILED", error = e),
        };

        let ids = collection::list_document_ids(
            &self.storage.config,
            &self.space,
            &self.db,
            collection,
            None,
            None,
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

    pub async fn index_document(
        &mut self,
        collection: &str,
        new_doc: &JsonValue,
    ) -> RaiseResult<()> {
        let Some(doc_id) = new_doc.get("_id").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_DB_DOCUMENT_ID_MISSING",
                error = "Impossible d'indexer le document : attribut '_id' manquant ou invalide.",
                context = json_value!({
                    "collection": collection,
                    "action": "index_document",
                    "hint": "Assurez-vous que le document est passé par le SchemaValidator (qui génère l'_id) avant d'atteindre le moteur d'indexation."
                })
            );
        };
        let indexes = self.load_indexes(collection).await?;

        for def in indexes {
            self.dispatch_update(collection, &def, doc_id, None, Some(new_doc))
                .await?;
        }
        Ok(())
    }

    pub async fn remove_document(
        &mut self,
        collection: &str,
        old_doc: &JsonValue,
    ) -> RaiseResult<()> {
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
        let content = fs::read_to_string_async(path).await?;
        json::deserialize_from_str(&content)
    }

    async fn dispatch_update(
        &self,
        col: &str,
        def: &IndexDefinition,
        id: &str,
        old: Option<&JsonValue>,
        new: Option<&JsonValue>,
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
                context = json_value!({ "index_name": def.name })
            );
        }
        Ok(())
    }
    pub async fn apply_indexes_from_config(&self, index_doc: &JsonValue) -> RaiseResult<()> {
        // Astuce Rust : On instancie un manager mutable localement car create_index exige &mut self
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);

        if let Some(collections) = index_doc.get("collections").and_then(|c| c.as_object()) {
            for (col_name, col_config) in collections {
                // On cherche la directive d'infrastructure
                if let Some(indexes) = col_config.get("x_indexes").and_then(|i| i.as_array()) {
                    for idx_field in indexes {
                        if let Some(field_name) = idx_field.as_str() {
                            if !self.has_index(col_name, field_name).await {
                                user_info!(
                                    "INDEX_SYNC",
                                    json_value!({ "col": col_name, "field": field_name, "action": "building" })
                                );

                                idx_mgr.create_index(col_name, field_name, "hash").await?;
                            } else {
                                user_trace!(
                                    "INDEX_OK",
                                    json_value!({ "col": col_name, "field": field_name })
                                );
                            }
                        }
                    }
                }
            }
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
        let content = fs::read_to_string_async(&meta_path).await?;
        json::deserialize_from_str(&content)?
    } else {
        CollectionMeta {
            schema: None,
            indexes: vec![],
        }
    };

    if !meta.indexes.iter().any(|i| i.name == def.name) {
        meta.indexes.push(def);
        fs::write_json_atomic_async(&meta_path, &meta).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::JsonDbConfig;

    #[async_test]
    async fn test_manager_lifecycle() -> RaiseResult<()> {
        // 🎯 FIX : Ajout du type de retour
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Fail TempDir: {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // 🎯 FIX : Extraction de l'instance (StorageEngine::new est faillible)
        let storage = match StorageEngine::new(config) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/users");
        fs::create_dir_all_async(&col_path).await?; // 🎯 FIX : Utilisation de '?'

        mgr.create_index("users", "email", "hash").await?;
        assert!(col_path.join("_meta.json").exists());

        let doc = json_value!({ "_id": "u1", "email": "a@a.com" });
        mgr.index_document("users", &doc).await?;

        let idx_path = col_path.join("_indexes/email.hash.idx");
        assert!(idx_path.exists());

        mgr.drop_index("users", "email").await?;
        assert!(!idx_path.exists());

        Ok(()) // 🎯 FIX : Succès
    }

    #[async_test]
    async fn test_manager_search_flow() -> RaiseResult<()> {
        // 🎯 FIX : Ajout du type de retour
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Fail TempDir: {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        let storage = match StorageEngine::new(config) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/products");
        fs::create_dir_all_async(&col_path).await?;

        mgr.create_index("products", "category", "hash").await?;

        let p1 = json_value!({ "_id": "p1", "category": "book" });
        let p2 = json_value!({ "_id": "p2", "category": "food" });

        mgr.index_document("products", &p1).await?;
        mgr.index_document("products", &p2).await?;

        assert!(mgr.has_index("products", "category").await);

        let results = mgr
            .search("products", "category", &json_value!("book"))
            .await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "p1");

        Ok(())
    }

    #[async_test]
    async fn test_list_indexes_filtering() -> RaiseResult<()> {
        // 🎯 FIX : Ajout du type de retour
        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Fail TempDir: {:?}", e),
        };
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        let storage = match StorageEngine::new(config) {
            Ok(s) => s,
            Err(e) => return Err(e),
        };
        let mut mgr = IndexManager::new(&storage, "s", "d");

        let col_path = dir.path().join("s/d/collections/users");
        fs::create_dir_all_async(&col_path).await?;

        mgr.create_index("users", "email", "hash").await?;
        mgr.create_index("users", "age", "btree").await?;

        let all_indexes = mgr.list_indexes("users", None).await?;
        assert_eq!(all_indexes.len(), 2);

        let email_index = mgr.list_indexes("users", Some("email")).await?;
        assert_eq!(email_index.len(), 1);
        assert_eq!(email_index[0].name, "email");

        Ok(())
    }
}
