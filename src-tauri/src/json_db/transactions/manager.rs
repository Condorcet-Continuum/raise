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

use crate::utils::prelude::*;

/// Structure pour stocker l'inverse d'une opération réalisée (Undo Log en mémoire)
enum UndoAction {
    Insert {
        collection: String,
        id: String,
        inserted_doc: JsonValue,
    },
    Update {
        collection: String,
        id: String,
        previous_doc: JsonValue,
        bad_doc: JsonValue,
    },
    Delete {
        collection: String,
        id: String,
        previous_doc: JsonValue,
    },
}

pub struct TransactionManager<'a> {
    storage: &'a StorageEngine,
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
    /// API PUBLIQUE INTELLIGENTE (ASYNCHRONE)
    pub async fn execute_smart(&self, requests: Vec<TransactionRequest>) -> RaiseResult<()> {
        let mut prepared_ops = Vec::new();

        let col_mgr = CollectionsManager::new(self.storage, &self.space, &self.db);
        let query_engine = QueryEngine::new(&col_mgr);

        #[cfg(debug_assertions)]
        println!("⚙️  [Manager] Traitement transaction étendu...");

        for req in requests {
            match req {
                TransactionRequest::Insert {
                    collection,
                    id,
                    mut document,
                } => {
                    // 🎯 FIX : Ajout de &query_engine et de .await?
                    self.resolve_all_refs(&query_engine, &mut document, &prepared_ops)
                        .await?;

                    if let Some(explicit_id) = id {
                        if let Some(obj) = document.as_object_mut() {
                            obj.insert("_id".to_string(), JsonValue::String(explicit_id));
                        }
                    }
                    col_mgr.prepare_document(&collection, &mut document).await?;

                    let final_id = match document.get("_id").and_then(|v| v.as_str()) {
                        Some(id_str) => id_str.to_string(),
                        None => {
                            raise_error!(
                                "ERR_TX_MISSING_ID",
                                error = "Le document préparé ne contient pas d'identifiant '_id'.",
                                context = json_value!({"collection": collection, "action": "prepare_insert_op"})
                            );
                        }
                    };

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
                    mut document,
                } => {
                    // 🎯 FIX
                    self.resolve_all_refs(&query_engine, &mut document, &prepared_ops)
                        .await?;

                    let final_id = self
                        .resolve_id(
                            &query_engine,
                            &collection,
                            id,
                            handle,
                            Some(&document),
                            &prepared_ops,
                        )
                        .await?;
                    prepared_ops.push(Operation::Update {
                        collection,
                        id: final_id,
                        previous_document: None,
                        document,
                    });
                }

                TransactionRequest::Upsert {
                    collection,
                    id,
                    handle,
                    mut document,
                } => {
                    // 1. Résolution sémantique (transformation des URI en UUIDs)
                    self.resolve_all_refs(&query_engine, &mut document, &prepared_ops)
                        .await?;

                    // 2. Extraction de l'identité sémantique résolue
                    let semantic_id = document
                        .get("@id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.starts_with("db://") && !s.starts_with("ref:"))
                        .map(|s| s.to_string());

                    // 🎯 VERROU D'INTÉGRITÉ 1 : Détection de conflit d'identité
                    if let (Some(prov_id), Some(sem_id)) = (&id, &semantic_id) {
                        if prov_id != sem_id {
                            raise_error!(
                                "ERR_DB_IDENTITY_CONFLICT",
                                error = "Conflit d'identité : l'ID fourni ne correspond pas à l'URI @id résolue.",
                                context = json_value!({ "provided": prov_id, "semantic": sem_id, "collection": collection })
                            );
                        }
                    }

                    let target_id_hint = semantic_id.or(id);

                    // 3. Tentative de résolution de l'identité physique
                    // On utilise match pour traiter l'erreur (Not Found) comme signal d'insertion
                    let resolution = self
                        .resolve_id(
                            &query_engine,
                            &collection,
                            target_id_hint.clone(),
                            handle,
                            Some(&document),
                            &prepared_ops,
                        )
                        .await;

                    match resolution {
                        Ok(existing_id) => {
                            // L'entité existe -> UPDATE
                            prepared_ops.push(Operation::Update {
                                collection,
                                id: existing_id,
                                previous_document: None,
                                document,
                            });
                        }
                        Err(_) => {
                            // L'entité n'existe pas -> INSERT
                            // 🎯 VERROU D'INTÉGRITÉ 2 : Injection de l'ID cible avant la préparation
                            // pour empêcher le x_compute (validator) de générer un ID divergent.
                            if let Some(tid) = target_id_hint {
                                if let Some(obj) = document.as_object_mut() {
                                    obj.insert("_id".to_string(), json_value!(tid));
                                }
                            }

                            let mut doc = document;
                            col_mgr.prepare_document(&collection, &mut doc).await?;

                            let final_id = match doc.get("_id").and_then(|v| v.as_str()) {
                                Some(id_str) => id_str.to_string(),
                                None => {
                                    raise_error!(
                                        "ERR_TX_MISSING_ID",
                                        error = "Le document préparé pour insertion est orphelin (pas de '_id').",
                                        context = json_value!({"collection": collection})
                                    );
                                }
                            };

                            prepared_ops.push(Operation::Insert {
                                collection,
                                id: final_id,
                                document: doc,
                            });
                        }
                    }
                }
                TransactionRequest::Delete { collection, id } => {
                    prepared_ops.push(Operation::Delete {
                        collection,
                        id,
                        previous_document: None,
                    });
                }
                TransactionRequest::InsertFrom { collection, path } => {
                    let mut doc = self.load_dataset_file(&path).await?;

                    self.resolve_all_refs(&query_engine, &mut doc, &prepared_ops)
                        .await?;

                    col_mgr.prepare_document(&collection, &mut doc).await?;

                    let final_id = match doc.get("_id").and_then(|v| v.as_str()) {
                        Some(id_str) => id_str.to_string(),
                        None => {
                            raise_error!(
                                "ERR_TX_MISSING_ID",
                                error = "Le document source ne contient pas d'identifiant '_id'.",
                                context = json_value!({"collection": collection, "action": "prepare_insert_from_op"})
                            );
                        }
                    };

                    prepared_ops.push(Operation::Insert {
                        collection,
                        id: final_id,
                        document: doc,
                    });
                }
                TransactionRequest::UpdateFrom { collection, path } => {
                    let mut doc = self.load_dataset_file(&path).await?;

                    // 🎯 FIX
                    self.resolve_all_refs(&query_engine, &mut doc, &prepared_ops)
                        .await?;

                    let handle = doc
                        .get("handle")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    let id_in_doc = doc
                        .get("_id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let final_id = self
                        .resolve_id(
                            &query_engine,
                            &collection,
                            id_in_doc,
                            handle,
                            Some(&doc),
                            &prepared_ops,
                        )
                        .await?;

                    prepared_ops.push(Operation::Update {
                        collection,
                        id: final_id,
                        previous_document: None,
                        document: doc,
                    });
                }
                TransactionRequest::UpsertFrom { collection, path } => {
                    let mut doc = self.load_dataset_file(&path).await?;
                    self.resolve_all_refs(&query_engine, &mut doc, &prepared_ops)
                        .await?;

                    let handle = doc
                        .get("handle")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let semantic_id = doc
                        .get("@id")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.starts_with("db://") && !s.starts_with("ref:"))
                        .map(|s| s.to_string());

                    let resolution = self
                        .resolve_id(
                            &query_engine,
                            &collection,
                            semantic_id.clone(),
                            handle,
                            Some(&doc),
                            &prepared_ops,
                        )
                        .await;

                    match resolution {
                        Ok(existing_id) => {
                            prepared_ops.push(Operation::Update {
                                collection,
                                id: existing_id,
                                previous_document: None,
                                document: doc,
                            });
                        }
                        Err(_) => {
                            // Injection de l'identité sémantique comme ID technique cible
                            if let Some(tid) = semantic_id {
                                if let Some(obj) = doc.as_object_mut() {
                                    obj.insert("_id".to_string(), json_value!(tid));
                                }
                            }

                            col_mgr.prepare_document(&collection, &mut doc).await?;
                            let new_id = match doc.get("_id").and_then(|v| v.as_str()) {
                                Some(id_str) => id_str.to_string(),
                                None => raise_error!(
                                    "ERR_TX_MISSING_ID",
                                    context = json_value!({"path": path.clone()})
                                ),
                            };
                            prepared_ops.push(Operation::Insert {
                                collection,
                                id: new_id,
                                document: doc,
                            });
                        }
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

    async fn load_dataset_file(&self, path: &str) -> RaiseResult<JsonValue> {
        let dataset_root = self
            .storage
            .config
            .data_root
            .join("dataset")
            .to_string_lossy()
            .to_string();

        let resolved_path = path.replace("$PATH_RAISE_DATASET", &dataset_root);

        let content = match fs::read_to_string_async(Path::new(&resolved_path)).await {
            Ok(c) => c,
            Err(e) => {
                raise_error!(
                    "ERR_FS_READ_FAIL",
                    error = format!("Échec de lecture du fichier : {}", e),
                    context = json_value!({
                        "resolved_path": resolved_path,
                        "os_error": e.to_string(),
                        "action": "load_collection_file",
                        "hint": "Vérifiez que le fichier existe et que l'application possède les droits de lecture."
                    })
                );
            }
        };
        json::deserialize_from_str(&content)
    }

    async fn resolve_id(
        &self,
        qe: &QueryEngine<'_>,
        collection: &str,
        id: Option<String>,
        handle: Option<String>,
        document: Option<&JsonValue>,
        pending_ops: &[Operation],
    ) -> RaiseResult<String> {
        // 1. Extraction des identités candidates
        let target_handle = handle.or_else(|| {
            document.and_then(|d| {
                d.get("handle")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
        });

        let target_id = id.or_else(|| {
            document.and_then(|d| {
                d.get("_id")
                    .or_else(|| d.get("@id"))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
        });

        let target_name = document.and_then(|d| {
            d.get("name").and_then(|v| {
                if v.is_string() {
                    v.as_str().map(|s| s.to_string())
                } else {
                    v.get("fr")
                        .or(v.get("en"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string())
                }
            })
        });

        // 🎯 NOTE : On retire le bloc "Early Return" qui était ici à la ligne 364.

        // 2. Recherche dans la RAM (Opérations en attente dans la transaction)
        for op in pending_ops.iter().rev() {
            if let Operation::Insert {
                collection: c,
                id: op_id,
                document: d,
            }
            | Operation::Update {
                collection: c,
                id: op_id,
                document: d,
                ..
            } = op
            {
                if c == collection {
                    if let Some(ref h) = target_handle {
                        if d.get("handle").and_then(|v| v.as_str()) == Some(h) {
                            return Ok(op_id.clone());
                        }
                    }
                    if let Some(ref n) = target_name {
                        let d_name = d.get("name").and_then(|v| {
                            if v.is_string() {
                                v.as_str().map(|s| s.to_string())
                            } else {
                                v.get("fr")
                                    .or(v.get("en"))
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string())
                            }
                        });
                        if d_name == Some(n.clone()) {
                            return Ok(op_id.clone());
                        }
                    }
                }
            }
        }

        // 3. Recherche sur le DISQUE (Vérification physique systématique 🎯)

        // A. Par ID technique (UUID résolu ou fourni)
        if let Some(ref i) = target_id {
            if !i.starts_with("db://")
                && !i.starts_with("ref:")
                && self
                    .storage
                    .read_document(&self.space, &self.db, collection, i)
                    .await
                    .ok()
                    .flatten()
                    .is_some()
            {
                return Ok(i.clone());
            }
        }

        // B. Par Handle (via Index + Vérification physique)
        if let Some(ref h) = target_handle {
            let query = Query {
                collection: collection.to_string(),
                filter: Some(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![Condition::eq("handle", json_value!(h))],
                }),
                sort: None,
                limit: Some(1),
                offset: None,
                projection: None,
            };

            if let Ok(res) = qe.execute_query(query).await {
                if let Some(found_doc) = res.documents.first() {
                    if let Some(id_str) = found_doc.get("_id").and_then(|v| v.as_str()) {
                        let id_to_check = id_str.to_string();
                        if self
                            .storage
                            .read_document(&self.space, &self.db, collection, &id_to_check)
                            .await
                            .ok()
                            .flatten()
                            .is_some()
                        {
                            return Ok(id_to_check);
                        } else {
                            raise_error!(
                                "ERR_DB_GHOST_ENTITY",
                                error = id_to_check, // On passe l'ID fantôme ici
                                context = json_value!({ "collection": collection, "handle": h })
                            );
                        }
                    }
                }
            }
        }

        // C. Par Name (via Index + Vérification physique)
        if let Some(ref n) = target_name {
            let query = Query {
                collection: collection.to_string(),
                filter: Some(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![Condition::eq("name", json_value!(n))],
                }),
                sort: None,
                limit: Some(1),
                offset: None,
                projection: None,
            };
            if let Ok(res) = qe.execute_query(query).await {
                if let Some(found_doc) = res.documents.first() {
                    if let Some(id_str) = found_doc.get("_id").and_then(|v| v.as_str()) {
                        if self
                            .storage
                            .read_document(&self.space, &self.db, collection, id_str)
                            .await
                            .ok()
                            .flatten()
                            .is_some()
                        {
                            return Ok(id_str.to_string());
                        }
                    }
                }
            }
        }

        raise_error!(
            "ERR_DB_IDENTITY_NOT_FOUND",
            error = format!(
                "Aucune entité physique trouvée pour '{}' dans '{}'",
                target_handle
                    .as_deref()
                    .or(target_name.as_deref())
                    .unwrap_or("unknown"),
                collection
            )
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

        let collections_to_lock: UniqueSet<String> = tx
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

        // Personne ne peut modifier ces fichiers pendant qu'on les lit.
        for op in &mut tx.operations {
            match op {
                Operation::Update {
                    collection,
                    id,
                    ref mut previous_document,
                    ..
                } => {
                    *previous_document = self
                        .storage
                        .read_document(&self.space, &self.db, collection, id)
                        .await
                        .unwrap_or(None);
                }
                Operation::Delete {
                    collection,
                    id,
                    ref mut previous_document,
                } => {
                    *previous_document = self
                        .storage
                        .read_document(&self.space, &self.db, collection, id)
                        .await
                        .unwrap_or(None);
                }
                _ => {}
            }
        }

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
        fs::ensure_dir_async(&wal_path).await?;
        let tx_file = wal_path.join(format!("{}.json", tx.id));
        fs::write_json_atomic_async(&tx_file, tx).await?;
        Ok(())
    }

    async fn apply_transaction(&self, tx: &Transaction) -> RaiseResult<()> {
        let mut idx = IndexManager::new(self.storage, &self.space, &self.db);

        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        let mut system_index = if sys_path.exists() {
            let c = fs::read_to_string_async(&sys_path).await?;
            json::deserialize_from_str::<JsonValue>(&c).unwrap_or(json_value!({"collections": {} }))
        } else {
            json_value!({"collections": {} })
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
                        if !obj.contains_key("_id") {
                            obj.insert("_id".to_string(), JsonValue::String(id.clone()));
                        }
                    }

                    if let Err(e) = self.apply_schema_logic(collection, &mut final_doc).await {
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

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
                    ..
                } => {
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
                                context = json_value!({
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
                        obj.insert("_id".to_string(), JsonValue::String(id.clone()));
                    }

                    if let Err(e) = self.apply_schema_logic(collection, &mut final_doc).await {
                        self.rollback_runtime(&mut idx, undo_stack).await?;
                        return Err(e);
                    }

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

                Operation::Delete { collection, id, .. } => {
                    let existing_opt = self
                        .storage
                        .read_document(&self.space, &self.db, collection, id)
                        .await
                        .ok()
                        .flatten();

                    if let Some(old_doc) = existing_opt {
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

        fs::write_json_atomic_async(&sys_path, &system_index).await?;
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

    async fn apply_schema_logic(&self, collection: &str, doc: &mut JsonValue) -> RaiseResult<()> {
        let meta_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");
        let mut resolved_uri = None;

        if meta_path.exists() {
            if let Ok(content) = fs::read_to_string_async(&meta_path).await {
                if let Ok(meta) = json::deserialize_from_str::<JsonValue>(&content) {
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
                obj.insert("$schema".to_string(), JsonValue::String(uri.clone()));
            }
            let mut target_space = self.space.clone();
            let mut target_db = self.db.clone();

            if let Some(without_scheme) = uri.strip_prefix("db://") {
                let parts: Vec<&str> = without_scheme.splitn(3, '/').collect();
                if parts.len() >= 2 {
                    target_space = parts[0].to_string();
                    target_db = parts[1].to_string();
                }
            }
            let reg =
                SchemaRegistry::from_db(&self.storage.config, &target_space, &target_db).await?;
            let validator = match SchemaValidator::compile_with_registry(&uri, &reg) {
                Ok(v) => v,
                Err(e) => {
                    raise_error!(
                        "ERR_SCHEMA_VALIDATOR_COMPILATION_FAIL",
                        error = format!(
                            "Impossible de préparer le validateur pour le schéma : {}",
                            uri
                        ),
                        context = json_value!({
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
        system_index: &mut JsonValue,
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
                cols.insert(
                    collection.to_string(),
                    json_value!({ "schema": "", "items": [] }),
                );
            }
            if let Some(col_data) = cols.get_mut(collection) {
                if col_data.get("items").is_none() {
                    if let Some(obj) = col_data.as_object_mut() {
                        obj.insert("items".to_string(), json_value!([]));
                    }
                }
                if let Some(items) = col_data.get_mut("items").and_then(|i| i.as_array_mut()) {
                    if is_delete {
                        items.retain(|i| i.get("file").and_then(|f| f.as_str()) != Some(&filename));
                    } else {
                        let exists = items
                            .iter()
                            .any(|i| i.get("file").and_then(|f| f.as_str()) == Some(&filename));
                        if !exists {
                            items.push(json_value!({ "file": filename }));
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
        if fs::exists_async(&path).await {
            fs::remove_file_async(&path).await?;
        }
        Ok(())
    }

    async fn rollback_wal(&self, tx: &Transaction) -> RaiseResult<()> {
        self.commit_wal(tx).await
    }

    #[async_recursive]
    async fn resolve_all_refs(
        &self,
        qe: &QueryEngine<'_>,
        doc: &mut JsonValue,
        pending_ops: &[Operation],
    ) -> RaiseResult<()> {
        let mut new_val = None;

        match doc {
            JsonValue::Object(map) => {
                for (_, v) in map.iter_mut() {
                    self.resolve_all_refs(qe, v, pending_ops).await?;
                }
            }
            JsonValue::Array(arr) => {
                for v in arr.iter_mut() {
                    self.resolve_all_refs(qe, v, pending_ops).await?;
                }
            }
            JsonValue::String(s) => {
                let mut parsed_target = None;

                // 1. Syntaxe LOCALE (ref:...)
                if s.starts_with("ref:") {
                    // splitn(4) garantit que si la valeur contient des ':', ils ne sont pas coupés
                    let parts: Vec<&str> = s.splitn(4, ':').collect();
                    if parts.len() == 4 {
                        parsed_target = Some((
                            self.space.as_str(),
                            self.db.as_str(),
                            parts[1],             // collection
                            parts[2],             // field
                            parts[3].to_string(), // value
                        ));
                    }
                }
                // 2. Syntaxe CROSS-DB (db://...)
                else if let Some(path) = s.strip_prefix("db://") {
                    let parts: Vec<&str> = path.splitn(5, '/').collect();
                    if parts.len() == 5 {
                        parsed_target = Some((
                            parts[0],             // domain
                            parts[1],             // db
                            parts[2],             // collection
                            parts[3],             // field
                            parts[4].to_string(), // value
                        ));
                    }
                }

                if let Some((t_domain, t_db, t_col, t_field, t_val)) = parsed_target {
                    // 🔍 A. PHASE RAM (Intra-Transaction) - Uniquement si c'est la DB courante
                    if t_domain == self.space && t_db == self.db {
                        for op in pending_ops.iter().rev() {
                            if let Operation::Insert {
                                collection,
                                id,
                                document,
                            }
                            | Operation::Update {
                                collection,
                                id,
                                document,
                                ..
                            } = op
                            {
                                if collection == t_col {
                                    if let Some(val) =
                                        document.get(t_field).and_then(|v| v.as_str())
                                    {
                                        if val == t_val {
                                            new_val = Some(id.clone());
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    // 💾 B. PHASE DISQUE (Extra-Transaction Locale OU Cross-DB)
                    if new_val.is_none() {
                        let q = Query {
                            collection: t_col.to_string(),
                            filter: Some(QueryFilter {
                                operator: FilterOperator::And,
                                conditions: vec![Condition {
                                    field: t_field.to_string(),
                                    operator: ComparisonOperator::Eq,
                                    value: JsonValue::String(t_val),
                                }],
                            }),
                            sort: None,
                            limit: Some(1),
                            offset: None,
                            projection: None,
                        };

                        if t_domain == self.space && t_db == self.db {
                            // Cible Locale : On réutilise le moteur de la transaction en cours
                            if let Ok(res) = qe.execute_query(q).await {
                                if let Some(found_doc) = res.documents.first() {
                                    if let Some(db_id) =
                                        found_doc.get("_id").and_then(|v| v.as_str())
                                    {
                                        new_val = Some(db_id.to_string());
                                    }
                                }
                            }
                        } else {
                            // Cible Distante (Cross-DB) : On instancie un moteur éphémère !
                            let ext_col_mgr = CollectionsManager::new(self.storage, t_domain, t_db);
                            let ext_qe = QueryEngine::new(&ext_col_mgr);

                            if let Ok(res) = ext_qe.execute_query(q).await {
                                if let Some(found_doc) = res.documents.first() {
                                    if let Some(db_id) =
                                        found_doc.get("_id").and_then(|v| v.as_str())
                                    {
                                        new_val = Some(db_id.to_string());
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }

        if let Some(id) = new_val {
            *doc = JsonValue::String(id);
        }

        Ok(())
    }
}

fn json_merge(a: &mut JsonValue, b: JsonValue) {
    match (a, b) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            for (k, v) in b {
                json_merge(a.entry(k).or_insert(JsonValue::Null), v);
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
    use crate::utils::testing::DbSandbox;

    async fn create_dataset_file(
        root: &Path,
        rel_path: &str,
        content: JsonValue,
    ) -> RaiseResult<()> {
        let full_path = root.join(rel_path);
        if let Some(parent) = full_path.parent() {
            fs::ensure_dir_async(parent).await?;
        }
        fs::write_json_atomic_async(&full_path, &content).await?;
        Ok(())
    }

    #[async_test]
    async fn test_transaction_commit_success() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let tm = TransactionManager::new(storage, space, db);
        let res = tm
            .execute(|tx| {
                tx.add_insert("users", "user1", json_value!({"name": "Alice"}));
                Ok(())
            })
            .await;

        assert!(res.is_ok());

        let doc_path = storage
            .config
            .db_collection_path(space, db, "users")
            .join("user1.json");
        assert!(fs::exists_async(&doc_path).await);
        Ok(())
    }

    #[async_test]
    async fn test_transaction_rollback_on_error() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let tm = TransactionManager::new(storage, space, db);
        let res = tm
            .execute(|tx| {
                tx.add_insert("users", "user2", json_value!({"name": "Bob"}));
                raise_error!(
                    "ERR_TX_SIMULATED_FAILURE",
                    error = "Échec intentionnel pour test de rollback",
                    context = json_value!({
                        "transaction_id": "test_sync_001"
                    })
                );
            })
            .await;

        assert!(res.is_err());

        let doc_path = storage
            .config
            .db_collection_path(space, db, "users")
            .join("user2.json");
        assert!(
            !fs::exists_async(&doc_path).await,
            "Le rollback a échoué, le fichier a été écrit !"
        );
        Ok(())
    }

    #[async_test]
    async fn test_smart_insert_injects_metadata() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let tm = TransactionManager::new(storage, space, db);
        let req = vec![TransactionRequest::Insert {
            collection: "users".to_string(),
            id: None,
            document: json_value!({ "name": "Test User" }),
        }];
        tm.execute_smart(req).await?;

        let res = QueryEngine::new(&col_mgr)
            .execute_query(Query::new("users"))
            .await?;
        assert_eq!(res.documents.len(), 1);
        assert!(res.documents[0].get("_id").is_some());
        Ok(())
    }

    #[async_test]
    async fn test_atomicity_failure_rollback_smart() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "items",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let mut idx_mgr = IndexManager::new(storage, space, db);
        idx_mgr.create_index("items", "val", "hash").await?;

        let tm = TransactionManager::new(storage, space, db);

        let req = vec![
            TransactionRequest::Insert {
                collection: "items".to_string(),
                id: Some("item1".to_string()),
                document: json_value!({ "_id": "item1","val": "A" }),
            },
            TransactionRequest::Update {
                collection: "items".to_string(),
                id: Some("ghost_id".to_string()),
                handle: None,
                document: json_value!({ "val": "B" }),
            },
        ];

        let result = tm.execute_smart(req).await;
        assert!(result.is_err(), "La transaction aurait dû échouer");

        let doc_path = storage
            .config
            .db_collection_path(space, db, "items")
            .join("item1.json");
        assert!(
            !fs::exists_async(&doc_path).await,
            "Rollback Fichier échoué : item1 ne devrait pas être là"
        );

        let search_res = idx_mgr.search("items", "val", &json_value!("A")).await?;
        assert!(
            search_res.is_empty(),
            "Rollback Index échoué : L'index contient encore la donnée !"
        );
        Ok(())
    }

    #[async_test]
    async fn test_upsert_workflow() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "actors",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let tm = TransactionManager::new(storage, space, db);

        let dataset_dir = match sandbox.config.get_path("PATH_RAISE_DOMAIN") {
            Some(p) => p.join("dataset"),
            None => {
                raise_error!(
                    "ERR_TEST_FAILURE",
                    error = "PATH_RAISE_DOMAIN non défini dans la Sandbox"
                );
            }
        };
        fs::ensure_dir_async(&dataset_dir).await?;

        create_dataset_file(
            &dataset_dir,
            "bob.json",
            json_value!({ "handle": "bob", "role": "worker" }),
        )
        .await?;

        let req1 = vec![TransactionRequest::UpsertFrom {
            collection: "actors".to_string(),
            path: "$PATH_RAISE_DATASET/bob.json".to_string(),
        }];
        tm.execute_smart(req1).await?;

        create_dataset_file(
            &dataset_dir,
            "bob.json",
            json_value!({ "handle": "bob", "role": "boss" }),
        )
        .await?;

        let req2 = vec![TransactionRequest::UpsertFrom {
            collection: "actors".to_string(),
            path: "$PATH_RAISE_DATASET/bob.json".to_string(),
        }];
        tm.execute_smart(req2).await?;

        let res = QueryEngine::new(&col_mgr)
            .execute_query(Query::new("actors"))
            .await?;

        assert_eq!(res.documents.len(), 1);
        assert_eq!(res.documents[0]["role"], "boss");
        Ok(())
    }

    #[async_test]
    async fn test_upsert_resolution_by_name() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let tm = TransactionManager::new(storage, space, db);

        let req1 = vec![TransactionRequest::Insert {
            collection: "users".to_string(),
            id: None,
            document: json_value!({
                "name": "alice",
                "handle": "alice_raise",
                "email": "alice@raise.local",
                "default_domain": "_system"
            }),
        }];

        tm.execute_smart(req1).await?;

        let req2 = vec![TransactionRequest::Update {
            collection: "users".to_string(),
            id: None,
            handle: None,
            document: json_value!({
                "name": "alice",
                "email": "new_alice@raise.local"
            }),
        }];

        let res = tm.execute_smart(req2).await;
        assert!(res.is_ok(), "Le moteur aurait dû résoudre l'identité");

        let engine = QueryEngine::new(&col_mgr);
        let query_res = engine.execute_query(Query::new("users")).await?;

        assert_eq!(query_res.documents.len(), 1);
        assert_eq!(query_res.documents[0]["email"], "new_alice@raise.local");
        Ok(())
    }

    #[async_test]
    async fn test_omni_reference_resolution() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let storage = &sandbox.storage;

        // 🌍 1. INITIALISATION CROSS-DB (Base Distante : _system/raise)
        let ext_domain = "_system";
        let ext_db = "raise";
        let ext_col_mgr = CollectionsManager::new(storage, ext_domain, ext_db);
        DbSandbox::mock_db(&ext_col_mgr).await?;
        ext_col_mgr
            .create_collection(
                "permissions",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // On injecte la permission cible dans la base distante
        let perm_doc =
            json_value!({ "_id": "uuid-ext-perm-999", "handle": "perm_model_engine_update_sa" });
        ext_col_mgr.insert_raw("permissions", &perm_doc).await?;

        // 🏠 2. INITIALISATION LOCALE (Base Courante : _system/bootstrap)
        let local_domain = "_system";
        let local_db = "bootstrap";
        let local_col_mgr = CollectionsManager::new(storage, local_domain, local_db);
        DbSandbox::mock_db(&local_col_mgr).await?;
        local_col_mgr
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;
        local_col_mgr
            .create_collection(
                "dapps",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;
        local_col_mgr
            .create_collection(
                "configs",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // On injecte l'utilisateur cible dans la base locale (Disque)
        let user_doc = json_value!({ "_id": "uuid-db-user-123", "handle": "admin" });
        local_col_mgr.insert_raw("users", &user_doc).await?;

        // ⚡ 3. LA TRANSACTION HYBRIDE
        let tm = TransactionManager::new(storage, local_domain, local_db);
        let reqs = vec![
            // A. Création dans la RAM Locale (Intra-Transaction)
            TransactionRequest::Insert {
                collection: "dapps".to_string(),
                id: Some("uuid-ram-dapp-456".to_string()),
                document: json_value!({ "handle": "raise_app" }),
            },
            // B. Document avec les 3 types de références !
            TransactionRequest::Insert {
                collection: "configs".to_string(),
                id: Some("uuid-config-789".to_string()),
                document: json_value!({
                    // Doit résoudre sur le Disque Local
                    "owner_id": "ref:users:handle:admin",

                    // Doit résoudre dans la RAM de la Transaction
                    "active_dapp_id": "ref:dapps:handle:raise_app",

                    // 🎯 Doit résoudre en Cross-DB via la création d'un QueryEngine éphémère
                    "perm_id": "db://_system/raise/permissions/handle/perm_model_engine_update_sa"
                }),
            },
        ];

        tm.execute_smart(reqs).await?;

        // 🔍 4. LES ASSERTIONS
        let config_doc = local_col_mgr
            .get("configs", "uuid-config-789")
            .await?
            .expect("La config doit exister");

        assert_eq!(
            config_doc["owner_id"], "uuid-db-user-123",
            "❌ Échec Ref Locale Disque"
        );
        assert_eq!(
            config_doc["active_dapp_id"], "uuid-ram-dapp-456",
            "❌ Échec Ref Locale RAM"
        );
        assert_eq!(
            config_doc["perm_id"], "uuid-ext-perm-999",
            "❌ Échec Ref CROSS-DB (db://...) !"
        );

        Ok(())
    }
}
