// FICHIER : src-tauri/src/commands/json_db_commands.rs

use crate::json_db::collections::manager::{self, CollectionsManager};
use crate::json_db::query::{sql::SqlRequest, Query, QueryEngine, QueryResult};
use crate::json_db::schema::SchemaRegistry;
use crate::json_db::storage::{file_storage, StorageEngine};
use crate::json_db::transactions::manager::TransactionManager;
use serde_json::{json, Value};
use tauri::{command, State};

// Helper pour instancier le manager rapidement
fn mgr<'a>(
    storage: &'a State<'_, StorageEngine>,
    space: &str,
    db: &str,
) -> Result<CollectionsManager<'a>, String> {
    Ok(CollectionsManager::new(storage, space, db))
}

// --- GESTION DATABASE ---

#[command]
pub async fn jsondb_create_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<(), String> {
    file_storage::create_db(&storage.config, &space, &db)
        .await
        .map_err(|e| e.to_string())?;
    let manager = mgr(&storage, &space, &db)?;
    manager.init_db().await.map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_drop_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<(), String> {
    file_storage::drop_db(&storage.config, &space, &db, file_storage::DropMode::Hard)
        .await
        .map_err(|e| e.to_string())
}

// --- GESTION COLLECTIONS ---

#[command]
pub async fn jsondb_create_collection(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    schema_uri: Option<String>,
) -> Result<(), String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .create_collection(&collection, schema_uri)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_list_collections(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<Vec<String>, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager.list_collections().await.map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_drop_collection(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
) -> Result<(), String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .drop_collection(&collection)
        .await
        .map_err(|e| e.to_string())
}

// --- GESTION INDEXES ---

#[command]
pub async fn jsondb_create_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
    kind: String,
) -> Result<(), String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .create_index(&collection, &field, &kind)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_drop_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
) -> Result<(), String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .drop_index(&collection, &field)
        .await
        .map_err(|e| e.to_string())
}

// --- MOTEUR DE RÈGLES ---

#[command]
pub async fn jsondb_evaluate_draft(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    mut doc: Value,
) -> Result<Value, String> {
    let registry = SchemaRegistry::from_db(&storage.config, &space, &db)
        .await
        .map_err(|e| format!("Erreur chargement registre: {}", e))?;
    let meta_path = storage
        .config
        .db_collection_path(&space, &db, &collection)
        .join("_meta.json");

    let schema_uri = if meta_path.exists() {
        let content = std::fs::read_to_string(&meta_path).map_err(|e| e.to_string())?;
        let meta: Value = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        meta.get("schema")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string()
    } else {
        return Err(format!(
            "Collection '{}' non initialisée (pas de _meta.json)",
            collection
        ));
    };

    if schema_uri.is_empty() {
        return Ok(doc);
    }

    let manager = mgr(&storage, &space, &db)?;

    manager::apply_business_rules(
        &manager,
        &collection,
        &mut doc,
        None,
        &registry,
        &schema_uri,
    )
    .await
    .map_err(|e| format!("Erreur exécution règles: {}", e))?;

    Ok(doc)
}

// --- CRUD DOCUMENTS ---

#[command]
pub async fn jsondb_insert_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    document: Value,
) -> Result<Value, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .insert_with_schema(&collection, document)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_update_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
    document: Value,
) -> Result<Value, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .update_document(&collection, &id, document)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_get_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
) -> Result<Option<Value>, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .get_document(&collection, &id)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_delete_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
) -> Result<bool, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .delete_document(&collection, &id)
        .await
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_list_all(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
) -> Result<Vec<Value>, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .list_all(&collection)
        .await
        .map_err(|e| format!("List All Failed: {}", e))
}

// --- REQUÊTES (MODIFIÉ POUR INSERT SQL) ---

#[command]
pub async fn jsondb_execute_query(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    query: Query,
) -> Result<QueryResult, String> {
    let manager = mgr(&storage, &space, &db)?;
    let engine = QueryEngine::new(&manager);
    engine.execute_query(query).await.map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_execute_sql(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    sql: String,
) -> Result<QueryResult, String> {
    let manager = mgr(&storage, &space, &db)?;

    // Parsing SQL -> SqlRequest (Read ou Write)
    let request = crate::json_db::query::sql::parse_sql(&sql)
        .map_err(|e| format!("SQL Parse Error: {}", e))?;

    match request {
        // CAS LECTURE (SELECT)
        SqlRequest::Read(query) => {
            let engine = QueryEngine::new(&manager);
            engine.execute_query(query).await.map_err(|e| e.to_string())
        }
        // CAS ÉCRITURE (INSERT)
        SqlRequest::Write(requests) => {
            // On instancie le TransactionManager pour gérer l'atomicité
            let tx_mgr = TransactionManager::new(&storage.config, &space, &db);
            tx_mgr
                .execute_smart(requests)
                .await
                .map_err(|e| format!("Transaction SQL Error: {}", e))?;

            // Retour d'un résultat vide mais valide
            Ok(QueryResult {
                documents: vec![],
                total_count: 0,
                limit: None,
                offset: None,
            })
        }
    }
}

// --- UTILITAIRES DÉMO ---

#[command]
pub async fn jsondb_init_demo_rules(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<(), String> {
    let mgr = mgr(&storage, &space, &db)?;
    mgr.init_db().await.map_err(|e| e.to_string())?;

    mgr.create_collection("users", None)
        .await
        .map_err(|e| e.to_string())?;
    let user_doc = json!({ "id": "u_dev", "name": "Alice Dev", "tjm": 500.0 });
    mgr.insert_raw("users", &user_doc)
        .await
        .map_err(|e| e.to_string())?;

    let schema_content = json!({
        "type": "object",
        "properties": {
            "user_id": { "type": "string" },
            "days": { "type": "number" },
            "created_at": { "type": "string" },
            "total": { "type": "number" },
            "due_at": { "type": "string" },
            "ref": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "calc_total_lookup",
                "target": "total",
                "expr": {
                    "mul": [
                        { "var": "days" },
                        { "lookup": { "collection": "users", "id": { "var": "user_id" }, "field": "tjm" } }
                    ]
                }
            },
            {
                "id": "calc_due_date",
                "target": "due_at",
                "expr": { "date_add": { "date": { "var": "created_at" }, "days": { "val": 30 } } }
            },
            {
                "id": "gen_ref",
                "target": "ref",
                "expr": {
                    "concat": [
                        { "val": "INV-" },
                        { "upper": { "var": "user_id" } },
                        { "val": "-" },
                        { "var": "total" }
                    ]
                }
            }
        ]
    });

    let schema_path = storage
        .config
        .db_schemas_root(&space, &db)
        .join("v1/invoices/default.json");
    if let Some(parent) = schema_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &schema_path,
        serde_json::to_string_pretty(&schema_content).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    let schema_uri = format!("db://{}/{}/schemas/v1/invoices/default.json", space, db);
    mgr.create_collection("invoices", Some(schema_uri))
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[command]
pub async fn jsondb_init_model_rules(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<(), String> {
    let mgr = mgr(&storage, &space, &db)?;
    mgr.init_db().await.map_err(|e| e.to_string())?;

    let schema_content = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "parent_pkg": { "type": "string" },
            "description": { "type": "string" },
            "full_path": { "type": "string" },
            "compliance": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "compute_path",
                "target": "full_path",
                "expr": {
                    "concat": [
                        { "var": "parent_pkg" },
                        { "val": "::" },
                        { "var": "name" }
                    ]
                }
            },
            {
                "id": "check_naming",
                "target": "compliance",
                "expr": {
                    "if": {
                        "condition": {
                            "regex_match": {
                                "value": { "var": "name" },
                                "pattern": { "val": "^LF_[A-Z0-9_]+$" }
                            }
                        },
                        "then_branch": { "val": "✅ VALIDE" },
                        "else_branch": { "val": "❌ NON_CONFORME (Doit commencer par LF_ et être en MAJ)" }
                    }
                }
            }
        ]
    });

    let schema_path = storage
        .config
        .db_schemas_root(&space, &db)
        .join("v1/la/functions.json");
    if let Some(parent) = schema_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &schema_path,
        serde_json::to_string_pretty(&schema_content).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    let schema_uri = format!("db://{}/{}/schemas/v1/la/functions.json", space, db);
    let _ = mgr
        .create_collection("logical_functions", Some(schema_uri))
        .await;

    Ok(())
}
