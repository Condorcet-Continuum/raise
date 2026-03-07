// FICHIER : src-tauri/src/json_db/collections/mod.rs

//! Façade Collections : API haut niveau pour manipuler les documents
//! 🚀 V2 : Utilisation persistante du StorageEngine pour conserver le cache LRU.

pub mod collection;
pub mod data_provider;
pub mod manager;

// FAÇADE UNIQUE
use crate::utils::io::PathBuf;
use crate::utils::prelude::*;

use crate::json_db::{
    collections::manager::CollectionsManager,
    schema::{SchemaRegistry, SchemaValidator},
    storage::{JsonDbConfig, StorageEngine},
};

// --- Helpers privés ---
fn collection_from_schema_rel(schema_rel: &str) -> String {
    let first_part = schema_rel.split('/').next().unwrap_or("default");

    if first_part.ends_with(".json") {
        first_part.trim_end_matches(".json").to_string()
    } else {
        first_part.to_string()
    }
}

// --- API Publique (Facade) ---

pub async fn create_collection(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
) -> RaiseResult<()> {
    // Délégation simple, ne nécessite pas de cache pour créer un dossier
    collection::create_collection_if_missing(&storage.config, space, db, collection_name).await
}

pub async fn drop_collection(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
) -> RaiseResult<()> {
    collection::drop_collection(&storage.config, space, db, collection_name).await
}

pub async fn insert_with_schema(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    schema_rel: &str,
    mut doc: Value,
) -> RaiseResult<Value> {
    let reg = SchemaRegistry::from_db(&storage.config, space, db).await?;
    let root_uri = reg.uri(schema_rel);
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)?;

    let collection_name = collection_from_schema_rel(schema_rel);

    // ✅ On réutilise l'instance persistante du StorageEngine !
    let manager = CollectionsManager::new(storage, space, db);

    if let Err(e) =
        manager::apply_business_rules(&manager, &collection_name, &mut doc, None, &reg, &root_uri)
            .await
    {
        raise_error!(
            "ERR_DB_BUSINESS_RULES_EXEC",
            error = e,
            context = json!({
                "collection": collection_name,
                "root_uri": root_uri,
                "action": "execute_business_rules_logic"
            })
        );
    }
    validator.compute_then_validate(&mut doc)?;

    let Some(id) = doc.get("_id").and_then(|v| v.as_str()) else {
        raise_error!(
            "ERR_DB_DOCUMENT_ID_MISSING",
            error = "Identifiant '_id' manquant ou n'est pas une chaîne de caractères",
            context = json!({
                "expected_field": "_id",
                "available_keys": doc.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                "action": "extract_document_id"
            })
        );
    };
    collection::update_document(storage, space, db, &collection_name, id, &doc).await?;
    Ok(doc)
}

pub async fn insert_raw(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
    doc: &Value,
) -> RaiseResult<()> {
    let Some(id) = doc.get("_id").and_then(|v| v.as_str()) else {
        raise_error!(
            "ERR_DB_DOCUMENT_ID_MISSING",
            error = "Document invalide : le champ '_id' est manquant ou n'est pas une chaîne de caractères.",
            context = json!({
                "expected_field": "_id",
                "available_keys": doc.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                "action": "document_identity_check"
            })
        );
    };
    collection::create_document(storage, space, db, collection_name, id, doc).await
}

pub async fn update_with_schema(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    schema_rel: &str,
    mut doc: Value,
) -> RaiseResult<Value> {
    let reg = SchemaRegistry::from_db(&storage.config, space, db).await?;
    let root_uri = reg.uri(schema_rel);
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)?;

    validator.compute_then_validate(&mut doc)?;

    let collection_name = collection_from_schema_rel(schema_rel);
    let Some(id) = doc.get("_id").and_then(|v| v.as_str()) else {
        raise_error!(
            "ERR_DB_DOCUMENT_ID_MISSING",
            error = "Document invalide : le champ '_id' est manquant ou n'est pas une chaîne de caractères.",
            context = json!({
                "expected_field": "_id",
                "available_keys": doc.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                "action": "verify_document_identity"
            })
        );
    };
    collection::update_document(storage, space, db, &collection_name, id, &doc).await?;
    Ok(doc)
}

pub async fn update_raw(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
    doc: &Value,
) -> RaiseResult<()> {
    let Some(id) = doc.get("_id").and_then(|v| v.as_str()) else {
        raise_error!(
            "ERR_DB_DOCUMENT_ID_MISSING",
            error = "Document invalide : le champ '_id' est manquant ou n'est pas une chaîne de caractères.",
            context = json!({
                "expected_field": "_id",
                "available_keys": doc.as_object().map(|m| m.keys().collect::<Vec<_>>()),
                "action": "verify_document_identity"
            })
        );
    };
    collection::update_document(storage, space, db, collection_name, id, doc).await
}

pub async fn get(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
    id: &str,
) -> RaiseResult<Value> {
    collection::read_document(storage, space, db, collection_name, id).await
}

pub async fn delete(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
    id: &str,
) -> RaiseResult<()> {
    collection::delete_document(storage, space, db, collection_name, id).await
}

pub async fn list_ids(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
) -> RaiseResult<Vec<String>> {
    collection::list_document_ids(&storage.config, space, db, collection_name).await
}

pub async fn list_all(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection_name: &str,
) -> RaiseResult<Vec<Value>> {
    collection::list_documents(storage, space, db, collection_name).await
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
