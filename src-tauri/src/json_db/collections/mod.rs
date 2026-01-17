// FICHIER : src-tauri/src/json_db/collections/mod.rs

//! Façade Collections : API haut niveau pour manipuler les documents

pub mod collection;
pub mod data_provider;
pub mod manager;

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;

use crate::json_db::{
    collections::manager::CollectionsManager,
    schema::{SchemaRegistry, SchemaValidator},
    storage::{JsonDbConfig, StorageEngine},
};

// --- Helpers privés ---

fn collection_from_schema_rel(schema_rel: &str) -> String {
    let first_part = schema_rel.split('/').next().unwrap_or("default");

    // CORRECTION : Si c'est un fichier racine (ex: "simple.json"), on retire l'extension.
    // Sinon (ex: "users/user.json"), on garde le nom du dossier ("users").
    if first_part.ends_with(".json") {
        first_part.trim_end_matches(".json").to_string()
    } else {
        first_part.to_string()
    }
}

// --- API Publique (Facade) ---

pub async fn create_collection(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<()> {
    collection::create_collection_if_missing(cfg, space, db, collection).await
}

pub async fn drop_collection(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<()> {
    collection::drop_collection(cfg, space, db, collection).await
}

pub async fn insert_with_schema(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    schema_rel: &str,
    mut doc: Value,
) -> Result<Value> {
    let reg = SchemaRegistry::from_db(cfg, space, db)?;
    let root_uri = reg.uri(schema_rel);
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)?;

    let collection_name = collection_from_schema_rel(schema_rel);

    let storage = StorageEngine::new(cfg.clone());
    let manager = CollectionsManager::new(&storage, space, db);

    // Migration async : ajout de .await
    manager::apply_business_rules(&manager, &collection_name, &mut doc, None, &reg, &root_uri)
        .await
        .context("Rules Engine Execution")?;

    validator.compute_then_validate(&mut doc)?;

    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Document ID manquant"))?;

    collection::update_document(cfg, space, db, &collection_name, id, &doc).await?;
    Ok(doc)
}

pub async fn insert_raw(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc: &Value,
) -> Result<()> {
    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Document sans ID"))?;

    collection::create_document(cfg, space, db, collection, id, doc).await
}

pub async fn update_with_schema(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    schema_rel: &str,
    mut doc: Value,
) -> Result<Value> {
    let reg = SchemaRegistry::from_db(cfg, space, db)?;
    let root_uri = reg.uri(schema_rel);
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)?;

    validator.compute_then_validate(&mut doc)?;

    let collection_name = collection_from_schema_rel(schema_rel);
    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Document ID manquant"))?;

    collection::update_document(cfg, space, db, &collection_name, id, &doc).await?;
    Ok(doc)
}

pub async fn update_raw(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc: &Value,
) -> Result<()> {
    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Document ID manquant"))?;

    collection::update_document(cfg, space, db, collection, id, doc).await
}

pub async fn get(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<Value> {
    collection::read_document(cfg, space, db, collection, id).await
}

pub async fn delete(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<()> {
    collection::delete_document(cfg, space, db, collection, id).await
}

pub async fn list_ids(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<String>> {
    collection::list_document_ids(cfg, space, db, collection).await
}

pub async fn list_all(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<Value>> {
    collection::list_documents(cfg, space, db, collection).await
}

pub fn db_root_path(cfg: &JsonDbConfig, space: &str, db: &str) -> PathBuf {
    cfg.db_root(space, db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helper_collection_from_schema() {
        assert_eq!(collection_from_schema_rel("users/user.json"), "users");
        assert_eq!(
            collection_from_schema_rel("invoices/2023/inv.json"),
            "invoices"
        );
        assert_eq!(collection_from_schema_rel("simple.json"), "simple");
    }
}
