// FICHIER : src-tauri/src/json_db/transactions/manager.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::indexes::IndexManager;
use crate::json_db::query::{
    ComparisonOperator, Condition, FilterOperator, Query, QueryEngine, QueryFilter,
};
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::StorageEngine;
use crate::json_db::transactions::lock_manager::LockManager;
use crate::json_db::transactions::{Operation, Transaction, TransactionRequest};

use crate::utils::data::HashSet;
use crate::utils::io::{self, Path};
use crate::utils::json;
use crate::utils::prelude::*;

use crate::utils::config::AppConfig;

/// Structure pour stocker l'inverse d'une opération réalisée (Undo Log en mémoire)
enum UndoAction {
    Insert {
        collection: String,
        id: String,
        inserted_doc: Value,
    },
    Update {
        collection: String,
        id: String,
        previous_doc: Value,
        bad_doc: Value,
    },
    Delete {
        collection: String,
        id: String,
        previous_doc: Value,
    },
}

pub struct TransactionManager<'a> {
    storage: &'a StorageEngine, // ✅ MODIFICATION : Injection du StorageEngine
    space: String,
    db: String,
    lock_manager: LockManager,
}

impl<'a> TransactionManager<'a> {
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            storage,
            space: space.to_string(),
            db: db.to_string(),
            lock_manager: LockManager::new(),
        }
    }

    /// API PUBLIQUE INTELLIGENTE (ASYNCHRONE)
    pub async fn execute_smart(&self, requests: Vec<TransactionRequest>) -> RaiseResult<()> {
        let mut prepared_ops = Vec::new();

        // ✅ MODIFICATION : Utilisation de l'instance centralisée
        let col_mgr = CollectionsManager::new(self.storage, &self.space, &self.db);
        let query_engine = QueryEngine::new(&col_mgr);

        #[cfg(debug_assertions)]
        println!("⚙️  [Manager] Traitement transaction étendu...");

        for req in requests {
            match req {
                TransactionRequest::Insert {
                    collection,
                    id: _,
                    mut document,
                } => {
                    col_mgr.prepare_document(&collection, &mut document).await?;
                    let final_id = document
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap()
                        .to_string();
                    prepared_ops.push(Operation::Insert {
                        collection,
                        id: final_id,
                        document,
                    });
                }
                TransactionRequest::Update {
                    collection,
                    id,
                    handle,
                    document,
                } => {
                    let final_id = self
                        .resolve_id(&query_engine, &collection, id, handle, Some(&document))
                        .await?;
                    prepared_ops.push(Operation::Update {
                        collection,
                        id: final_id,
                        document,
                    });
                }

                TransactionRequest::Upsert {
                    collection,
                    id,
                    handle,
                    document,
                } => {
                    // On tente de résoudre l'ID. Si ça échoue (ok()), on sait que c'est un Insert.
                    let found_id = self
                        .resolve_id(&query_engine, &collection, id, handle, Some(&document))
                        .await
                        .ok();

                    if let Some(existing_id) = found_id {
                        prepared_ops.push(Operation::Update {
                            collection,
                            id: existing_id,
                            document,
                        });
                    } else {
                        let mut doc = document;
                        col_mgr.prepare_document(&collection, &mut doc).await?;
                        let new_id = doc.get("id").and_then(|v| v.as_str()).unwrap().to_string();
                        prepared_ops.push(Operation::Insert {
                            collection,
                            id: new_id,
                            document: doc,
                        });
                    }
                }
                TransactionRequest::Delete { collection, id } => {
                    prepared_ops.push(Operation::Delete { collection, id });
                }
                TransactionRequest::InsertFrom { collection, path } => {
                    let mut doc = self.load_dataset_file(&path).await?;
                    col_mgr.prepare_document(&collection, &mut doc).await?;
                    let final_id = doc.get("id").and_then(|v| v.as_str()).unwrap().to_string();
                    prepared_ops.push(Operation::Insert {
                        collection,
                        id: final_id,
                        document: doc,
                    });
                }
                TransactionRequest::UpdateFrom { collection, path } => {
                    let doc = self.load_dataset_file(&path).await?;
                    let handle = doc
                        .get("handle")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let id_in_doc = doc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let final_id = self
                        .resolve_id(&query_engine, &collection, id_in_doc, handle, Some(&doc))
                        .await?;

                    prepared_ops.push(Operation::Update {
                        collection,
                        id: final_id,
                        document: doc,
                    });
                }
                TransactionRequest::UpsertFrom { collection, path } => {
                    let mut doc = self.load_dataset_file(&path).await?;
                    let handle = doc
                        .get("handle")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let id_in_doc = doc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let found_id = self
                        .resolve_id(&query_engine, &collection, id_in_doc, handle, Some(&doc))
                        .await
                        .ok();

                    if let Some(existing_id) = found_id {
                        prepared_ops.push(Operation::Update {
                            collection,
                            id: existing_id,
                            document: doc,
                        });
                    } else {
                        col_mgr.prepare_document(&collection, &mut doc).await?;
                        let new_id = doc.get("id").and_then(|v| v.as_str()).unwrap().to_string();
                        prepared_ops.push(Operation::Insert {
                            collection,
                            id: new_id,
                            document: doc,
                        });
                    }
                }
            }
        }

        self.execute_internal(|tx| {
            for op in prepared_ops {
                tx.operations.push(op);
            }
            Ok(())
        })
        .await
    }

    async fn load_dataset_file(&self, path: &str) -> RaiseResult<Value> {
        let config = AppConfig::get();
        let domain_path = config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("ERREUR: PATH_RAISE_DOMAIN introuvable dans la configuration !");

        let dataset_root = config
            .get_path("PATH_RAISE_DATASET")
            .unwrap_or_else(|| domain_path.join("dataset"))
            .to_string_lossy()
            .to_string();

        let resolved_path = path.replace("$PATH_RAISE_DATASET", &dataset_root);

        let content = match io::read_to_string(Path::new(&resolved_path)).await {
            Ok(c) => c,
            Err(e) => {
                raise_error!(
                    "ERR_FS_READ_FAIL",
                    error = format!("Échec de lecture du fichier : {}", e),
                    context = json!({
                        "resolved_path": resolved_path,
                        "os_error": e.to_string(),
                        "action": "load_collection_file",
                        "hint": "Vérifiez que le fichier existe et que l'application possède les droits de lecture."
                    })
                );
            }
        };
        json::parse(&content)
    }

    async fn resolve_id(
        &self,
        qe: &QueryEngine<'_>,
        collection: &str,
        id: Option<String>,
        handle: Option<String>,
        document: Option<&Value>,
    ) -> RaiseResult<String> {
        if let Some(i) = id {
            return Ok(i);
        }
        if let Some(h) = handle {
            let q = Query {
                collection: collection.to_string(),
                filter: Some(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![Condition {
                        field: "handle".to_string(),
                        operator: ComparisonOperator::Eq,
                        value: Value::String(h),
                    }],
                }),
                sort: None,
                limit: Some(1),
                offset: None,
                projection: None,
            };
            let res = qe.execute_query(q).await?;
            if let Some(doc) = res.documents.first() {
                return Ok(doc.get("id").and_then(|v| v.as_str()).unwrap().to_string());
            }
        }
        if let Some(doc) = document {
            if let Some(name_val) = doc.get("name") {
                let q = Query {
                    collection: collection.to_string(),
                    filter: Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: vec![Condition::eq("name", name_val.clone())],
                    }),
                    sort: None,
                    limit: Some(1),
                    offset: None,
                    projection: None,
                };
                let res = qe.execute_query(q).await?;
                if let Some(found_doc) = res.documents.first() {
                    return Ok(found_doc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .unwrap()
                        .to_string());
                }
            }
        }
        raise_error!(
            "ERR_DB_IDENTITY_NOT_FOUND",
            error = format!(
                "Aucune entité trouvée pour l'identité fournie dans '{}'",
                collection
            ),
            context = json!({
                "collection": collection,
                "action": "resolve_entity_identity",
                "hint": "L'identifiant ou le handle spécifié n'existe pas dans cette collection."
            })
        );
    }

    pub async fn execute<F>(&self, op_block: F) -> RaiseResult<()>
    where
        F: FnOnce(&mut Transaction) -> RaiseResult<()>,
    {
        self.execute_internal(op_block).await
    }

    async fn execute_internal<F>(&self, op_block: F) -> RaiseResult<()>
    where
        F: FnOnce(&mut Transaction) -> RaiseResult<()>,
    {
        let mut tx = Transaction::new();
        op_block(&mut tx)?;

        // 1. VERROUILLAGE
        let collections_to_lock: HashSet<String> = tx
            .operations
            .iter()
            .map(|op| match op {
                Operation::Insert { collection, .. } => collection.clone(),
                Operation::Update { collection, .. } => collection.clone(),
                Operation::Delete { collection, .. } => collection.clone(),
            })
            .collect();

        let mut sorted_collections: Vec<String> = collections_to_lock.into_iter().collect();
        sorted_collections.sort();

        let mut locks = Vec::new();
        let mut _guards = Vec::new();

        for col in sorted_collections {
            locks.push(
                self.lock_manager
                    .get_write_lock(&self.space, &self.db, &col),
            );
        }

        for lock in &locks {
            _guards.push(lock.write().await);
        }

        // 2. EXÉCUTION ATOMIQUE
        self.write_wal(&tx).await?;

        match self.apply_transaction(&tx).await {
            Ok(_) => {
                self.commit_wal(&tx).await?;
                Ok(())
            }
            Err(e) => {
                self.rollback_wal(&tx).await?;
                Err(e)
            }
        }
    }

    async fn write_wal(&self, tx: &Transaction) -> RaiseResult<()> {
        let wal_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("wal");
        io::ensure_dir(&wal_path).await?;
        let tx_file = wal_path.join(format!("{}.json", tx.id));
        io::write_json_atomic(&tx_file, tx).await?;
        Ok(())
    }

    async fn apply_transaction(&self, tx: &Transaction) -> RaiseResult<()> {
        // ✅ MODIFICATION : On utilise self.storage au lieu d'en récréer un !
        let mut idx = IndexManager::new(self.storage, &self.space, &self.db);

        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        let mut system_index = if sys_path.exists() {
            let c = io::read_to_string(&sys_path).await?;
            json::parse::<Value>(&c).unwrap_or(json!({"collections": {} }))
        } else {
            json!({"collections": {} })
        };

        let mut undo_stack: Vec<UndoAction> = Vec::new();

        for op in &tx.operations {
            match op {
                Operation::Insert {
                    collection,
                    id,
                    document,
                } => {
                    let mut final_doc = document.clone();
                    if let Some(obj) = final_doc.as_object_mut() {
                        if !obj.contains_key("id") {
                            obj.insert("id".to_string(), Value::String(id.clone()));
                        }
                    }

                    if let Err(e) = self.apply_schema_logic(collection, &mut final_doc).await {
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

                    // ✅ MODIFICATION : Write-Through (Cache + Disque)
                    if let Err(e) = self
                        .storage
                        .write_document(&self.space, &self.db, collection, id, &final_doc)
                        .await
                    {
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

                    if let Err(e) = idx.index_document(collection, &final_doc).await {
                        self.storage
                            .delete_document(&self.space, &self.db, collection, id)
                            .await
                            .ok();
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

                    self.update_index_entry(&mut system_index, collection, id, false)?;
                    undo_stack.push(UndoAction::Insert {
                        collection: collection.clone(),
                        id: id.clone(),
                        inserted_doc: final_doc,
                    });
                }

                Operation::Update {
                    collection,
                    id,
                    document,
                } => {
                    // ✅ MODIFICATION : Lecture via le Cache LRU
                    let existing_opt = match self
                        .storage
                        .read_document(&self.space, &self.db, collection, id)
                        .await
                    {
                        Ok(d) => d,
                        Err(e) => {
                            self.rollback_runtime(&mut idx, undo_stack).await?;
                            return Err(e);
                        }
                    };

                    let mut final_doc = match existing_opt {
                        Some(d) => d,
                        None => {
                            self.rollback_runtime(&mut idx, undo_stack).await?;
                            raise_error!(
                                "ERR_DB_UPDATE_TARGET_MISSING",
                                error = format!("Impossible de mettre à jour le document {}/{} : cible introuvable.", collection, id),
                                context = json!({
                                    "collection": collection,
                                    "document_id": id,
                                    "action": "execute_update_op",
                                    "transaction_state": "rolled_back",
                                })
                            );
                        }
                    };

                    let old_doc_clone = final_doc.clone();
                    json_merge(&mut final_doc, document.clone());

                    if let Some(obj) = final_doc.as_object_mut() {
                        obj.insert("id".to_string(), Value::String(id.clone()));
                    }

                    if let Err(e) = self.apply_schema_logic(collection, &mut final_doc).await {
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

                    // ✅ MODIFICATION : Mise à jour Cache + Disque
                    if let Err(e) = self
                        .storage
                        .write_document(&self.space, &self.db, collection, id, &final_doc)
                        .await
                    {
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

                    if let Err(e) = idx.remove_document(collection, &old_doc_clone).await {
                        self.storage
                            .write_document(&self.space, &self.db, collection, id, &old_doc_clone)
                            .await
                            .ok();
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }
                    if let Err(e) = idx.index_document(collection, &final_doc).await {
                        idx.index_document(collection, &old_doc_clone).await.ok();
                        self.storage
                            .write_document(&self.space, &self.db, collection, id, &old_doc_clone)
                            .await
                            .ok();
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

                    self.update_index_entry(&mut system_index, collection, id, false)?;
                    undo_stack.push(UndoAction::Update {
                        collection: collection.clone(),
                        id: id.clone(),
                        previous_doc: old_doc_clone,
                        bad_doc: final_doc,
                    });
                }

                Operation::Delete { collection, id } => {
                    // ✅ MODIFICATION : Lecture via Cache
                    let existing_opt = self
                        .storage
                        .read_document(&self.space, &self.db, collection, id)
                        .await
                        .ok()
                        .flatten();

                    if let Some(old_doc) = existing_opt {
                        // ✅ MODIFICATION : Suppression Cache + Disque
                        if let Err(e) = self
                            .storage
                            .delete_document(&self.space, &self.db, collection, id)
                            .await
                        {
                            self.rollback_runtime(&mut idx, undo_stack).await?;
                            return Err(e);
                        }

                        if let Err(e) = idx.remove_document(collection, &old_doc).await {
                            self.storage
                                .write_document(&self.space, &self.db, collection, id, &old_doc)
                                .await
                                .ok();
                            self.rollback_runtime(&mut idx, undo_stack).await?;
                            return Err(e);
                        }

                        self.update_index_entry(&mut system_index, collection, id, true)?;
                        undo_stack.push(UndoAction::Delete {
                            collection: collection.clone(),
                            id: id.clone(),
                            previous_doc: old_doc,
                        });
                    }
                }
            }
        }

        io::write_json_atomic(&sys_path, &system_index).await?;
        Ok(())
    }

    async fn rollback_runtime(
        &self,
        idx: &mut IndexManager<'_>,
        undo_stack: Vec<UndoAction>,
    ) -> RaiseResult<()> {
        #[cfg(debug_assertions)]
        println!(
            "⚠️ Rollback en cours ({} opérations à annuler)...",
            undo_stack.len()
        );

        for action in undo_stack.into_iter().rev() {
            match action {
                UndoAction::Insert {
                    collection,
                    id,
                    inserted_doc,
                } => {
                    // ✅ MODIFICATION : Suppression via le StorageEngine
                    self.storage
                        .delete_document(&self.space, &self.db, &collection, &id)
                        .await
                        .ok();
                    idx.remove_document(&collection, &inserted_doc).await.ok();
                }
                UndoAction::Update {
                    collection,
                    id,
                    previous_doc,
                    bad_doc,
                } => {
                    self.storage
                        .write_document(&self.space, &self.db, &collection, &id, &previous_doc)
                        .await
                        .ok();
                    idx.remove_document(&collection, &bad_doc).await.ok();
                    idx.index_document(&collection, &previous_doc).await.ok();
                }
                UndoAction::Delete {
                    collection,
                    id,
                    previous_doc,
                } => {
                    self.storage
                        .write_document(&self.space, &self.db, &collection, &id, &previous_doc)
                        .await
                        .ok();
                    idx.index_document(&collection, &previous_doc).await.ok();
                }
            }
        }
        Ok(())
    }

    async fn apply_schema_logic(&self, collection: &str, doc: &mut Value) -> RaiseResult<()> {
        let meta_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");
        let mut resolved_uri = None;

        if meta_path.exists() {
            if let Ok(content) = io::read_to_string(&meta_path).await {
                if let Ok(meta) = json::parse::<Value>(&content) {
                    if let Some(s) = meta.get("schema").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            resolved_uri = Some(s.to_string());
                        }
                    }
                }
            }
        }

        if let Some(uri) = resolved_uri {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("$schema".to_string(), Value::String(uri.clone()));
            }
            let reg = SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db).await?;
            let validator = match SchemaValidator::compile_with_registry(&uri, &reg) {
                Ok(v) => v,
                Err(e) => {
                    raise_error!(
                        "ERR_SCHEMA_VALIDATOR_COMPILATION_FAIL",
                        error = format!(
                            "Impossible de préparer le validateur pour le schéma : {}",
                            uri
                        ),
                        context = json!({
                            "schema_uri": uri,
                            "nested_error": e,
                            "action": "initialize_validator",
                        })
                    );
                }
            };
            validator.compute_then_validate(doc)?;
        }
        Ok(())
    }

    fn update_index_entry(
        &self,
        system_index: &mut Value,
        collection: &str,
        id: &str,
        is_delete: bool,
    ) -> RaiseResult<()> {
        let filename = format!("{}.json", id);
        if let Some(cols) = system_index
            .get_mut("collections")
            .and_then(|c| c.as_object_mut())
        {
            if !cols.contains_key(collection) && !is_delete {
                cols.insert(collection.to_string(), json!({ "schema": "", "items": [] }));
            }
            if let Some(col_data) = cols.get_mut(collection) {
                if col_data.get("items").is_none() {
                    col_data
                        .as_object_mut()
                        .unwrap()
                        .insert("items".to_string(), json!([]));
                }
                if let Some(items) = col_data.get_mut("items").and_then(|i| i.as_array_mut()) {
                    if is_delete {
                        items.retain(|i| i.get("file").and_then(|f| f.as_str()) != Some(&filename));
                    } else {
                        let exists = items
                            .iter()
                            .any(|i| i.get("file").and_then(|f| f.as_str()) == Some(&filename));
                        if !exists {
                            items.push(json!({ "file": filename }));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    async fn commit_wal(&self, tx: &Transaction) -> RaiseResult<()> {
        let path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("wal")
            .join(format!("{}.json", tx.id));
        if io::exists(&path).await {
            io::remove_file(&path).await?;
        }
        Ok(())
    }

    async fn rollback_wal(&self, tx: &Transaction) -> RaiseResult<()> {
        self.commit_wal(tx).await
    }
}

fn json_merge(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                json_merge(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::io::{self, tempdir, Path};

    async fn setup_test_db() -> (tempfile::TempDir, JsonDbConfig, String, String) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig {
            data_root: dir.path().to_path_buf(),
        };
        let space = "test_space";
        let db = "test_db";
        let db_path = config.db_root(space, db);

        io::ensure_dir(&db_path).await.unwrap();
        io::write_json_atomic(&db_path.join("_system.json"), &json!({ "collections": {} }))
            .await
            .unwrap();

        (dir, config, space.to_string(), db.to_string())
    }

    async fn create_dataset_file(root: &Path, rel_path: &str, content: Value) {
        let full_path = root.join(rel_path);
        if let Some(parent) = full_path.parent() {
            io::ensure_dir(parent).await.unwrap();
        }
        io::write_json_atomic(&full_path, &content).await.unwrap();
    }

    #[tokio::test]
    async fn test_transaction_commit_success() {
        let (_dir, config, space, db) = setup_test_db().await;
        // ✅ On instancie le StorageEngine pour le test
        let storage = StorageEngine::new(config.clone());
        io::ensure_dir(&config.db_root(&space, &db).join("users"))
            .await
            .unwrap();
        let tm = TransactionManager::new(&storage, &space, &db);
        let res = tm
            .execute(|tx| {
                tx.add_insert("users", "user1", json!({"name": "Alice"}));
                Ok(())
            })
            .await;
        assert!(res.is_ok());
        assert!(
            io::exists(
                &config
                    .db_collection_path(&space, &db, "users")
                    .join("user1.json")
            )
            .await
        );
    }

    #[tokio::test]
    async fn test_transaction_rollback_on_error() {
        let (_dir, config, space, db) = setup_test_db().await;
        let storage = StorageEngine::new(config.clone());

        io::ensure_dir(&config.db_root(&space, &db).join("users"))
            .await
            .unwrap();

        let tm = TransactionManager::new(&storage, &space, &db);
        let res = tm
            .execute(|tx| {
                tx.add_insert("users", "user2", json!({"name": "Bob"}));
                raise_error!(
                    "ERR_TX_SIMULATED_FAILURE",
                    error = "Échec intentionnel pour test de rollback",
                    context = json!({
                        "transaction_id": "test_sync_001"
                    })
                );
            })
            .await;
        assert!(res.is_err());
        let doc_path = config
            .db_collection_path(&space, &db, "users")
            .join("user2.json");
        assert!(!doc_path.exists());
    }

    #[tokio::test]
    async fn test_smart_insert_injects_metadata() {
        let (_dir, config, space, db) = setup_test_db().await;
        let storage = StorageEngine::new(config.clone());
        let tm = TransactionManager::new(&storage, &space, &db);
        io::ensure_dir(&config.db_collection_path(&space, &db, "users"))
            .await
            .unwrap();
        let req = vec![TransactionRequest::Insert {
            collection: "users".to_string(),
            id: None,
            document: json!({ "name": "Test User" }),
        }];
        tm.execute_smart(req).await.expect("Transaction failed");

        let col_mgr = CollectionsManager::new(&storage, &space, &db);
        let res = QueryEngine::new(&col_mgr)
            .execute_query(Query::new("users"))
            .await
            .unwrap();
        assert_eq!(res.documents.len(), 1);
        assert!(res.documents[0].get("id").is_some());
    }

    #[tokio::test]
    async fn test_atomicity_failure_rollback_smart() {
        let (_dir, config, space, db) = setup_test_db().await;
        let storage = StorageEngine::new(config.clone());
        let tm = TransactionManager::new(&storage, &space, &db);

        let mut idx_mgr = IndexManager::new(&storage, &space, &db);

        io::ensure_dir(&config.db_collection_path(&space, &db, "items"))
            .await
            .unwrap();
        idx_mgr.create_index("items", "val", "hash").await.unwrap();

        let req = vec![
            TransactionRequest::Insert {
                collection: "items".to_string(),
                id: Some("item1".to_string()),
                document: json!({ "val": "A" }),
            },
            TransactionRequest::Update {
                collection: "items".to_string(),
                id: Some("ghost_id".to_string()),
                handle: None,
                document: json!({ "val": "B" }),
            },
        ];

        let result = tm.execute_smart(req).await;
        assert!(result.is_err(), "La transaction aurait dû échouer");

        let doc_path = config
            .db_collection_path(&space, &db, "items")
            .join("item1.json");
        assert!(
            !doc_path.exists(),
            "Rollback Fichier échoué : item1 ne devrait pas être là"
        );

        let search_res = idx_mgr.search("items", "val", &json!("A")).await.unwrap();
        assert!(
            search_res.is_empty(),
            "Rollback Index échoué : L'index contient encore la donnée !"
        );
    }

    #[tokio::test]
    async fn test_upsert_workflow() {
        let (_dir, config, space, db) = setup_test_db().await;
        AppConfig::init().ok();

        let storage = StorageEngine::new(config.clone());
        let tm = TransactionManager::new(&storage, &space, &db);
        io::ensure_dir(&config.db_collection_path(&space, &db, "actors"))
            .await
            .unwrap();

        let app_cfg = AppConfig::get();
        let domain_path = app_cfg
            .get_path("PATH_RAISE_DOMAIN")
            .expect("PATH_RAISE_DOMAIN doit être défini dans la sandbox");

        let dataset_dir = app_cfg
            .get_path("PATH_RAISE_DATASET")
            .unwrap_or_else(|| domain_path.join("dataset"));

        io::ensure_dir(&dataset_dir).await.unwrap();

        create_dataset_file(
            &dataset_dir,
            "bob.json",
            json!({ "handle": "bob", "role": "worker" }),
        )
        .await;

        let req1 = vec![TransactionRequest::UpsertFrom {
            collection: "actors".to_string(),
            path: "$PATH_RAISE_DATASET/bob.json".to_string(),
        }];
        tm.execute_smart(req1).await.unwrap();

        create_dataset_file(
            &dataset_dir,
            "bob.json",
            json!({ "handle": "bob", "role": "boss" }),
        )
        .await;

        let req2 = vec![TransactionRequest::UpsertFrom {
            collection: "actors".to_string(),
            path: "$PATH_RAISE_DATASET/bob.json".to_string(),
        }];
        tm.execute_smart(req2).await.unwrap();

        let res = QueryEngine::new(&CollectionsManager::new(&storage, &space, &db))
            .execute_query(Query::new("actors"))
            .await
            .unwrap();

        assert_eq!(res.documents.len(), 1);
        assert_eq!(res.documents[0]["role"], "boss");
    }

    #[tokio::test]
    async fn test_upsert_resolution_by_name() {
        let (_dir, config, space, db) = setup_test_db().await;
        let storage = StorageEngine::new(config.clone());
        let tm = TransactionManager::new(&storage, &space, &db);

        // 1. Initialisation de la collection
        io::ensure_dir(&config.db_collection_path(&space, &db, "users"))
            .await
            .unwrap();

        // 2. Premier passage : Insertion initiale d'Alice
        let req1 = vec![TransactionRequest::Insert {
            collection: "users".to_string(),
            id: None, // On laisse le moteur générer l'ID
            document: json!({
                "name": "alice",
                "username": "alice_raise",
                "email": "alice@raise.local",
                "default_domain": "_system"
            }),
        }];

        tm.execute_smart(req1)
            .await
            .expect("L'insertion initiale a échoué");

        // 3. Deuxième passage : "Upsert" par le nom
        // On change l'email, mais on ne donne ni ID ni Handle.
        // Le moteur DOIT trouver l'ID existant via le champ 'name'.
        let req2 = vec![TransactionRequest::Update {
            collection: "users".to_string(),
            id: None,
            handle: None,
            document: json!({
                "name": "alice",
                "email": "new_alice@raise.local"
            }),
        }];

        let res = tm.execute_smart(req2).await;
        assert!(
            res.is_ok(),
            "Le moteur aurait dû résoudre l'identité via le nom sans erreur"
        );

        // 4. Vérification finale : Avons-nous un seul document mis à jour ?
        let col_mgr = CollectionsManager::new(&storage, &space, &db);
        let engine = QueryEngine::new(&col_mgr);
        let query_res = engine.execute_query(Query::new("users")).await.unwrap();

        assert_eq!(
            query_res.documents.len(),
            1,
            "Il ne devrait y avoir qu'un seul document Alice"
        );
        assert_eq!(
            query_res.documents[0]["email"], "new_alice@raise.local",
            "L'email aurait dû être mis à jour"
        );
    }
}
