// FICHIER : src-tauri/src/commands/json_db_commands.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Query, QueryEngine, QueryResult};
use crate::json_db::storage::{file_storage, StorageEngine};
use serde_json::Value;
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
    // 1. Création physique + Schémas
    file_storage::create_db(&storage.config, &space, &db).map_err(|e| e.to_string())?;

    // 2. Initialisation logique (Manager)
    let manager = mgr(&storage, &space, &db)?;
    manager.init_db().map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_drop_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<(), String> {
    file_storage::drop_db(&storage.config, &space, &db, file_storage::DropMode::Hard)
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
        .map_err(|e| e.to_string())
}

#[command]
pub async fn jsondb_list_collections(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> Result<Vec<String>, String> {
    let manager = mgr(&storage, &space, &db)?;
    manager.list_collections().map_err(|e| e.to_string())
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
        .map_err(|e| e.to_string())
}

// --- GESTION INDEXES (NOUVEAU) ---

#[command]
pub async fn jsondb_create_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
    kind: String, // "hash", "btree", "text"
) -> Result<(), String> {
    let manager = mgr(&storage, &space, &db)?;
    manager
        .create_index(&collection, &field, &kind)
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
        .map_err(|e| e.to_string())
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
        .map_err(|e| format!("List All Failed: {}", e))
}

// --- REQUÊTES ---

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
    let query = crate::json_db::query::sql::parse_sql(&sql)
        .map_err(|e| format!("SQL Parse Error: {}", e))?;
    let engine = QueryEngine::new(&manager);
    engine.execute_query(query).await.map_err(|e| e.to_string())
}
