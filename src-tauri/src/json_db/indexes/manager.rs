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
                    "hint": "Vérifiez que le dossier de la collection n'a pas été déplacé ou supprimé."
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
                    "action": "index_resolution",
                    "hint": "Vérifiez que l'index a été correctement créé pour cette collection."
                })
            );
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
    ) -> RaiseResult<Vec<String>> {
        // Chargement async des définitions d'index
        let indexes = self.load_indexes(collection).await?;

        let Some(def) = indexes.iter().find(|i| i.name == field) else {
            raise_error!(
                "ERR_DB_INDEX_DEFINITION_NOT_FOUND",
                error = format!("Définition d'index introuvable pour le champ '{}'", field),
                context = json!({
                    "requested_field": field,
                    "available_indexes": indexes.iter().map(|i| &i.name).collect::<Vec<_>>(),
                    "action": "lookup_index_metadata"
                })
            );
        };

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

    async fn rebuild_index(&self, collection: &str, def: &IndexDefinition) -> RaiseResult<()> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);
        let indexes_dir = col_path.join("_indexes");
        io::create_dir_all(&indexes_dir).await?;

        let mut entries = io::read_dir(&col_path).await?;
        while let Some(entry) = match entries.next_entry().await {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_FS_ITERATION_FAIL",
                error = e,
                context = json!({ "path": col_path, "action": "sync_collection_files" })
            ),
        } {
            let path = entry.path();

            // Vérification de l'extension sans panique
            if path.extension().is_some_and(|e| e == "json") {
                // Sécurisation du nom de fichier (on évite le unwrap().to_str().unwrap())
                let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

                if filename.is_empty() || filename.starts_with('_') {
                    continue;
                }

                // Lecture du fichier (Propagera l'erreur si le fichier est verrouillé)
                let content = match io::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(e) => raise_error!(
                        "ERR_FS_READ_FAIL",
                        error = e,
                        context = json!({ "file_path": path })
                    ),
                };

                // Parsing et Dispatch
                if let Ok(doc) = json::parse::<Value>(&content) {
                    let doc_id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if !doc_id.is_empty() {
                        // Ici on laisse le ? car dispatch_update renvoie probablement déjà un RaiseResult
                        self.dispatch_update(collection, def, doc_id, None, Some(&doc))
                            .await?;
                    }
                }
            }
        }
        Ok(())
    }

    pub async fn index_document(&mut self, collection: &str, new_doc: &Value) -> RaiseResult<()> {
        let Some(doc_id) = new_doc.get("id").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_DB_DOCUMENT_ID_MISSING",
                error = "Document invalide : le champ 'id' est manquant ou n'est pas une chaîne de caractères.",
                context = json!({
                    "expected_field": "id",
                    "available_keys": new_doc.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                    "action": "insert_document_validation"
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
        let cfg = &self.storage.config;
        let s = &self.space;
        let d = &self.db;
        let result = match def.index_type {
            IndexType::Hash => hash::update_hash_index(cfg, s, d, col, def, id, old, new).await,
            IndexType::BTree => btree::update_btree_index(cfg, s, d, col, def, id, old, new).await,
            IndexType::Text => text::update_text_index(cfg, s, d, col, def, id, old, new).await,
        };

        if let Err(e) = result {
            raise_error!(
                "ERR_DB_INDEX_UPDATE_FAIL",
                error = e,
                context = json!({
                    "index_name": def.name,
                    "index_type": format!("{:?}", def.index_type),
                    "document_id": id,
                    "action": "sync_index_state"
                })
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

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

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
        io::create_dir_all(&col_path).await.unwrap();

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
