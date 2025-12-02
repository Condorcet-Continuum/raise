use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Query, QueryEngine, QueryResult};
use crate::json_db::storage::StorageEngine;
use serde_json::Value;
use tauri::{command, State};

// Helper pour instancier le manager rapidement
fn mgr<'a>(
    storage: &'a State<'_, StorageEngine>,
    space: &str,
    db: &str,
) -> Result<CollectionsManager<'a>, String> {
    // Ici, vous pouvez ajouter une validation si space/db n'existent pas
    Ok(CollectionsManager::new(storage, space, db))
}

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
pub async fn jsondb_insert_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    document: Value,
) -> Result<Value, String> {
    let manager = mgr(&storage, &space, &db)?;
    // Utilise insert_with_schema pour garantir les IDs et validations
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

    // Parsing SQL
    let query = crate::json_db::query::sql::parse_sql(&sql)
        .map_err(|e| format!("SQL Parse Error: {}", e))?;

    let engine = QueryEngine::new(&manager);
    engine.execute_query(query).await.map_err(|e| e.to_string())
}

// CELLE QUI MANQUAIT :
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
