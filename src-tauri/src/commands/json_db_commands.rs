//! JSON-DB Tauri commands
//!
//! Ces commandes exposent les opérations principales (CRUD) via Tauri.
//! NOTE: On utilise des imports `crate::...` car on est *dans* le même crate.
//! Les tests externes peuvent, eux, utiliser `genaptitude::...`.

use serde_json::Value;
use std::path::Path;

use crate::json_db::{
    collections::manager::CollectionsManager,
    storage::{file_storage, JsonDbConfig},
};

/// Construit une config à partir de l’arbo du repo (CARGO_MANIFEST_DIR = src-tauri/)
fn cfg_from_repo_env() -> Result<JsonDbConfig, String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "cannot resolve repo root".to_string())?;
    JsonDbConfig::from_env(repo_root).map_err(|e| e.to_string())
}

/// Helper pour obtenir un manager lié (space, db)
fn mgr(space: &str, db: &str) -> Result<(JsonDbConfig, CollectionsManager<'static>), String> {
    // On construit une config puis un manager qui l’emprunte.
    // Pour satisfaire les durées de vie, on "leake" la config en 'static'
    // (pattern simple et sûr ici, la config est petite et vit jusqu’à la fin du process).
    let cfg_owned = cfg_from_repo_env()?;
    let cfg_static: &'static JsonDbConfig = Box::leak(Box::new(cfg_owned));
    // S’assure que la DB existe
    file_storage::create_db(cfg_static, space, db).map_err(|e| e.to_string())?;
    let m = CollectionsManager::new(cfg_static, space, db);
    Ok((cfg_static.clone(), unsafe {
        // Safety: cfg_static est 'static via leak, on peut retourner un manager lié à 'static
        std::mem::transmute::<CollectionsManager<'_>, CollectionsManager<'static>>(m)
    }))
}

/// Crée une collection si manquante
#[tauri::command]
pub fn jsondb_create_collection(
    space: String,
    db: String,
    collection: String,
) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.create_collection(&collection).map_err(|e| e.to_string())
}

/// Supprime une collection (dossier)
#[tauri::command]
pub fn jsondb_drop_collection(space: String, db: String, collection: String) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.drop_collection(&collection).map_err(|e| e.to_string())
}

/// Insert avec schéma (x_compute + validate + $schema auto)
#[tauri::command]
pub fn jsondb_insert_with_schema(
    space: String,
    db: String,
    schema_rel: String,
    mut doc: Value,
) -> Result<Value, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.insert_with_schema(&schema_rel, doc.take())
        .map_err(|e| e.to_string())
}

/// Upsert avec schéma (insert sinon update)
#[tauri::command]
pub fn jsondb_upsert_with_schema(
    space: String,
    db: String,
    schema_rel: String,
    mut doc: Value,
) -> Result<Value, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.upsert_with_schema(&schema_rel, doc.take())
        .map_err(|e| e.to_string())
}

/// Insert brut (sans schéma)
#[tauri::command]
pub fn jsondb_insert_raw(
    space: String,
    db: String,
    collection: String,
    doc: Value,
) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.insert_raw(&collection, &doc).map_err(|e| e.to_string())
}

/// Update avec schéma (recompute + validate)
#[tauri::command]
pub fn jsondb_update_with_schema(
    space: String,
    db: String,
    schema_rel: String,
    mut doc: Value,
) -> Result<Value, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.update_with_schema(&schema_rel, doc.take())
        .map_err(|e| e.to_string())
}

/// Update brut (sans schéma)
#[tauri::command]
pub fn jsondb_update_raw(
    space: String,
    db: String,
    collection: String,
    doc: Value,
) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.update_raw(&collection, &doc).map_err(|e| e.to_string())
}

/// Lecture par id
#[tauri::command]
pub fn jsondb_get(
    space: String,
    db: String,
    collection: String,
    id: String,
) -> Result<Value, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.get(&collection, &id).map_err(|e| e.to_string())
}

/// Suppression par id
#[tauri::command]
pub fn jsondb_delete(
    space: String,
    db: String,
    collection: String,
    id: String,
) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.delete(&collection, &id).map_err(|e| e.to_string())
}

/// Liste des IDs d’une collection
#[tauri::command]
pub fn jsondb_list_ids(
    space: String,
    db: String,
    collection: String,
) -> Result<Vec<String>, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.list_ids(&collection).map_err(|e| e.to_string())
}

/// Liste de tous les documents d’une collection
#[tauri::command]
pub fn jsondb_list_all(
    space: String,
    db: String,
    collection: String,
) -> Result<Vec<Value>, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.list_all(&collection).map_err(|e| e.to_string())
}

/// (Optionnel) Rechargement du registre de schémas
#[tauri::command]
pub fn jsondb_refresh_registry(space: String, db: String) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.refresh_registry().map_err(|e| e.to_string())
}
