// FICHIER : src-tauri/src/json_db/transactions/manager.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::indexes::IndexManager;
use crate::json_db::query::{
    ComparisonOperator, Condition, FilterOperator, Query, QueryEngine, QueryFilter,
};
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::{file_storage, JsonDbConfig, StorageEngine};
use crate::json_db::transactions::lock_manager::LockManager;
use crate::json_db::transactions::{Operation, Transaction, TransactionRequest};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;

pub struct TransactionManager<'a> {
    config: &'a JsonDbConfig,
    space: String,
    db: String,
    lock_manager: LockManager,
}

impl<'a> TransactionManager<'a> {
    pub fn new(config: &'a JsonDbConfig, space: &str, db: &str) -> Self {
        Self {
            config,
            space: space.to_string(),
            db: db.to_string(),
            lock_manager: LockManager::new(),
        }
    }

    /// API PUBLIQUE INTELLIGENTE (ASYNCHRONE)
    pub async fn execute_smart(&self, requests: Vec<TransactionRequest>) -> Result<()> {
        let mut prepared_ops = Vec::new();

        let storage = StorageEngine::new(self.config.clone());
        let col_mgr = CollectionsManager::new(&storage, &self.space, &self.db);
        let query_engine = QueryEngine::new(&col_mgr);

        println!("⚙️  [Manager] Préparation intelligente de la transaction...");

        for req in requests {
            match req {
                TransactionRequest::Insert {
                    collection,
                    id,
                    mut document,
                } => {
                    let final_id = id.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                    // Injection préliminaire (sera confirmée par apply_transaction)
                    if let Some(obj) = document.as_object_mut() {
                        obj.insert("id".to_string(), Value::String(final_id.clone()));
                    }

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
                    let final_id = if let Some(i) = id {
                        i
                    } else if let Some(h) = handle {
                        let q = Query {
                            collection: collection.clone(),
                            filter: Some(QueryFilter {
                                operator: FilterOperator::And,
                                conditions: vec![Condition {
                                    field: "handle".to_string(),
                                    operator: ComparisonOperator::Eq,
                                    value: Value::String(h.clone()),
                                }],
                            }),
                            sort: None,
                            limit: Some(1),
                            offset: None,
                            projection: None,
                        };

                        let res = query_engine.execute_query(q).await?;

                        if let Some(doc) = res.documents.first() {
                            doc.get("id").and_then(|v| v.as_str()).unwrap().to_string()
                        } else {
                            return Err(anyhow!(
                                "Transaction annulée : Handle '{}' introuvable dans '{}'",
                                h,
                                collection
                            ));
                        }
                    } else {
                        return Err(anyhow!(
                            "Transaction Update invalide : 'id' ou 'handle' requis."
                        ));
                    };

                    prepared_ops.push(Operation::Update {
                        collection,
                        id: final_id,
                        document,
                    });
                }

                TransactionRequest::Delete { collection, id } => {
                    prepared_ops.push(Operation::Delete { collection, id });
                }

                TransactionRequest::InsertFrom { collection, path } => {
                    let dataset_root = std::env::var("PATH_GENAPTITUDE_DATASET")
                        .unwrap_or_else(|_| ".".to_string());
                    let resolved_path = path.replace("$PATH_GENAPTITUDE_DATASET", &dataset_root);

                    let content = fs::read_to_string(&resolved_path).with_context(|| {
                        format!("Impossible de lire le fichier : {}", resolved_path)
                    })?;

                    let mut doc: Value = serde_json::from_str(&content)?;
                    let id = doc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

                    if let Some(obj) = doc.as_object_mut() {
                        obj.insert("id".to_string(), Value::String(id.clone()));
                    }

                    prepared_ops.push(Operation::Insert {
                        collection,
                        id,
                        document: doc,
                    });
                }
            }
        }

        self.execute_internal(|tx| {
            for op in prepared_ops {
                tx.operations.push(op);
            }
            Ok(())
        })
    }

    /// API BAS NIVEAU (Interne / Tests)
    pub fn execute<F>(&self, op_block: F) -> Result<()>
    where
        F: FnOnce(&mut Transaction) -> Result<()>,
    {
        self.execute_internal(op_block)
    }

    fn execute_internal<F>(&self, op_block: F) -> Result<()>
    where
        F: FnOnce(&mut Transaction) -> Result<()>,
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
            _guards.push(lock.write().unwrap());
        }

        // 2. EXÉCUTION ATOMIQUE
        self.write_wal(&tx)?;
        match self.apply_transaction(&tx) {
            Ok(_) => {
                self.commit_wal(&tx)?;
                Ok(())
            }
            Err(e) => {
                self.rollback_wal(&tx)?;
                Err(e)
            }
        }
    }

    fn write_wal(&self, tx: &Transaction) -> Result<()> {
        let wal_path = self.config.db_root(&self.space, &self.db).join("wal");
        if !wal_path.exists() {
            fs::create_dir_all(&wal_path)?;
        }
        let tx_file = wal_path.join(format!("{}.json", tx.id));
        fs::write(tx_file, serde_json::to_string_pretty(tx)?)?;
        Ok(())
    }

    fn apply_transaction(&self, tx: &Transaction) -> Result<()> {
        let storage = StorageEngine::new(self.config.clone());
        let mut idx = IndexManager::new(&storage, &self.space, &self.db);

        let sys_path = self
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        let mut system_index = if sys_path.exists() {
            let c = fs::read_to_string(&sys_path)?;
            serde_json::from_str::<Value>(&c).unwrap_or(json!({ "collections": {} }))
        } else {
            json!({ "collections": {} })
        };

        for op in &tx.operations {
            match op {
                Operation::Insert {
                    collection,
                    id,
                    document,
                } => {
                    let mut final_doc = document.clone();

                    // CORRECTION CRITIQUE : Assurance que l'ID est dans le corps du document
                    // Cela évite l'erreur "Document sans ID" dans IndexManager
                    if let Some(obj) = final_doc.as_object_mut() {
                        if !obj.contains_key("id") {
                            obj.insert("id".to_string(), Value::String(id.clone()));
                        }
                    }

                    self.apply_schema_logic(collection, &mut final_doc)?;

                    file_storage::write_document(
                        self.config,
                        &self.space,
                        &self.db,
                        collection,
                        id,
                        &final_doc,
                    )?;
                    idx.index_document(collection, &final_doc)?;
                    self.update_index_entry(&mut system_index, collection, id, false)?;
                }
                Operation::Update {
                    collection,
                    id,
                    document,
                } => {
                    let existing_opt = file_storage::read_document(
                        self.config,
                        &self.space,
                        &self.db,
                        collection,
                        id,
                    )?;
                    let mut final_doc = existing_opt.ok_or_else(|| {
                        anyhow!("Update échoué : doc {}/{} introuvable", collection, id)
                    })?;

                    json_merge(&mut final_doc, document.clone());

                    // S'assurer que l'ID n'a pas été perdu ou corrompu par le merge
                    if let Some(obj) = final_doc.as_object_mut() {
                        if !obj.contains_key("id") {
                            obj.insert("id".to_string(), Value::String(id.clone()));
                        }
                    }

                    self.apply_schema_logic(collection, &mut final_doc)?;
                    file_storage::write_document(
                        self.config,
                        &self.space,
                        &self.db,
                        collection,
                        id,
                        &final_doc,
                    )?;
                    idx.index_document(collection, &final_doc)?;
                    self.update_index_entry(&mut system_index, collection, id, false)?;
                }
                Operation::Delete { collection, id } => {
                    file_storage::delete_document(
                        self.config,
                        &self.space,
                        &self.db,
                        collection,
                        id,
                    )?;
                    idx.remove_document(collection, &serde_json::Value::Null)?; // Idéalement faudrait lire l'ancien doc avant pour désindexer proprement
                    self.update_index_entry(&mut system_index, collection, id, true)?;
                }
            }
        }
        fs::write(&sys_path, serde_json::to_string_pretty(&system_index)?)?;
        Ok(())
    }

    fn apply_schema_logic(&self, collection: &str, doc: &mut Value) -> Result<()> {
        let meta_path = self
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");
        let schema_uri = if meta_path.exists() {
            let meta: Value = serde_json::from_str(&fs::read_to_string(&meta_path)?)?;
            meta.get("schema")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        if let Some(uri) = schema_uri {
            if !uri.is_empty() {
                if let Some(obj) = doc.as_object_mut() {
                    if !obj.contains_key("$schema")
                        || obj
                            .get("$schema")
                            .and_then(|s| s.as_str())
                            .unwrap_or("")
                            .is_empty()
                    {
                        obj.insert("$schema".to_string(), Value::String(uri.clone()));
                    }
                }
                let reg = SchemaRegistry::from_db(self.config, &self.space, &self.db)?;
                let validator = SchemaValidator::compile_with_registry(&uri, &reg)
                    .context(format!("Schema error: {}", uri))?;
                validator.compute_then_validate(doc)?;
            }
        }
        Ok(())
    }

    fn update_index_entry(
        &self,
        system_index: &mut Value,
        collection: &str,
        id: &str,
        is_delete: bool,
    ) -> Result<()> {
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

    fn commit_wal(&self, tx: &Transaction) -> Result<()> {
        let path = self
            .config
            .db_root(&self.space, &self.db)
            .join("wal")
            .join(format!("{}.json", tx.id));
        if path.exists() {
            fs::remove_file(path)?;
        }
        Ok(())
    }

    fn rollback_wal(&self, tx: &Transaction) -> Result<()> {
        self.commit_wal(tx)
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
