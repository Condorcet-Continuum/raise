// FICHIER : crates/raise-desktop/src/commands/json_db_commands.rs

use raise_core::json_db::query::QueryResult;
use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*;

// 🎯 On importe le service pur depuis le noyau
use raise_core::services::json_db_service;

use tauri::{command, State};

#[command]
pub async fn jsondb_create_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_create_db(storage.inner(), &space, &db).await
}

#[command]
pub async fn jsondb_drop_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_drop_db(storage.inner(), &space, &db).await
}

#[command]
pub async fn jsondb_create_collection(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    schema_uri: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_create_collection(
        storage.inner(),
        &space,
        &db,
        &collection,
        &schema_uri,
    )
    .await
}

#[command]
pub async fn jsondb_list_collections(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<Vec<String>> {
    json_db_service::jsondb_list_collections(storage.inner(), &space, &db).await
}

#[command]
pub async fn jsondb_drop_collection(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_drop_collection(storage.inner(), &space, &db, &collection).await
}

#[command]
pub async fn jsondb_create_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
    kind: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_create_index(storage.inner(), &space, &db, &collection, &field, &kind)
        .await
}

#[command]
pub async fn jsondb_drop_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_drop_index(storage.inner(), &space, &db, &collection, &field).await
}

#[command]
pub async fn jsondb_evaluate_draft(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    doc: JsonValue,
) -> RaiseResult<JsonValue> {
    json_db_service::jsondb_evaluate_draft(storage.inner(), &space, &db, &collection, doc).await
}

#[command]
pub async fn jsondb_insert_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    document: JsonValue,
) -> RaiseResult<JsonValue> {
    json_db_service::jsondb_insert_document(storage.inner(), &space, &db, &collection, document)
        .await
}

#[command]
pub async fn jsondb_update_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
    document: JsonValue,
) -> RaiseResult<JsonValue> {
    json_db_service::jsondb_update_document(
        storage.inner(),
        &space,
        &db,
        &collection,
        &id,
        document,
    )
    .await
}

#[command]
pub async fn jsondb_get_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
) -> RaiseResult<Option<JsonValue>> {
    json_db_service::jsondb_get_document(storage.inner(), &space, &db, &collection, &id).await
}

#[command]
pub async fn jsondb_delete_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
) -> RaiseResult<bool> {
    json_db_service::jsondb_delete_document(storage.inner(), &space, &db, &collection, &id).await
}

#[command]
pub async fn jsondb_list_all(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
) -> RaiseResult<Vec<JsonValue>> {
    json_db_service::jsondb_list_all(storage.inner(), &space, &db, &collection).await
}

#[command]
pub async fn jsondb_execute_sql(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    sql: String,
) -> RaiseResult<QueryResult> {
    json_db_service::jsondb_execute_sql(storage.inner(), &space, &db, &sql).await
}

#[command]
pub async fn jsondb_execute_query(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    query: raise_core::json_db::query::Query,
) -> RaiseResult<QueryResult> {
    json_db_service::jsondb_execute_query(storage.inner(), &space, &db, query).await
}

#[command]
pub async fn jsondb_init_demo_rules(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<()> {
    json_db_service::jsondb_init_demo_rules(storage.inner(), &space, &db).await
}
