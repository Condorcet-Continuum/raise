//! JSON-DB Tauri commands
//!
//! Ces commandes exposent les opérations principales (CRUD) via Tauri.

use serde_json::Value;
use std::path::Path;

// 2. QueryInput est dans json_db::query
use crate::json_db::query::{QueryEngine, QueryInput, QueryResult};

use crate::json_db::{
    collections::manager::CollectionsManager,
    storage::{file_storage, JsonDbConfig},
};
// -----------------------------

/// Construit une config à partir de l’arbo du repo (CARGO_MANIFEST_DIR = src-tauri/)
fn cfg_from_repo_env() -> Result<JsonDbConfig, String> {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| "cannot resolve repo root".to_string())?;
    JsonDbConfig::from_env(repo_root).map_err(|e| e.to_string())
}

/// Helper pour obtenir un manager lié (space, db)
/// Tente d'ouvrir la DB, et si elle n'existe pas, la crée.
fn mgr(space: &str, db: &str) -> Result<(JsonDbConfig, CollectionsManager<'static>), String> {
    // On construit une config puis un manager qui l’emprunte.
    // Pour satisfaire les durées de vie, on "leake" la config en 'static'
    let cfg_owned = cfg_from_repo_env()?;
    let cfg_static: &'static JsonDbConfig = Box::leak(Box::new(cfg_owned));

    // CORRECTION ICI : Logique "Open OR Create"
    // On essaie d'ouvrir. Si ça échoue (n'existe pas), on crée.
    if file_storage::open_db(cfg_static, space, db).is_err() {
        file_storage::create_db(cfg_static, space, db).map_err(|e| e.to_string())?;
    }

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

/// Insert avec schéma :
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

/// Upsert avec schéma
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

/// Insert direct (sans schéma)
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

/// Update avec schéma
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

/// Update direct (sans schéma)
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

/// Rechargement du registre de schémas
#[tauri::command]
pub fn jsondb_refresh_registry(space: String, db: String) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;
    m.refresh_registry().map_err(|e| e.to_string())
}

// ----------------------------------------------------------------------
// --- Fonctions Résolvant les Erreurs du main.rs et du moteur de requête ---
// ----------------------------------------------------------------------

/// Fonction de requête
#[tauri::command]
pub async fn jsondb_query_collection(
    space: String,
    db: String,
    _bucket: String,
    query_json: String,
) -> Result<QueryResult, String> {
    // 1. Désérialisation de la requête
    let query_input: QueryInput = match serde_json::from_str(&query_json) {
        Ok(q) => q,
        Err(e) => return Err(format!("Requête JSON invalide : {}", e)),
    };

    // 2. Initialisation de la DB via le manager
    let (_cfg, m) = mgr(&space, &db)?;

    // 3. Création du QueryEngine et exécution
    // 💡 CORRECTION : Ajout du & pour passer la référence
    let engine = QueryEngine::new(&m);

    // 💡 CORRECTION : Utilisation de la méthode correcte execute_query
    match engine.execute_query(query_input).await {
        Ok(result) => Ok(result),
        Err(e) => Err(format!(
            "Erreur d'exécution de la requête : {}",
            e.to_string()
        )),
    }
}

/// Insert simple (résout `__cmd__jsondb_insert` dans main.rs)
#[tauri::command]
pub fn jsondb_insert(
    space: String,
    db: String,
    schema_rel: String,
    doc: Value,
) -> Result<Value, String> {
    jsondb_insert_with_schema(space, db, schema_rel, doc)
}

/// Upsert simple (résout `__cmd__jsondb_upsert` dans main.rs)
#[tauri::command]
pub fn jsondb_upsert(
    space: String,
    db: String,
    schema_rel: String,
    doc: Value,
) -> Result<Value, String> {
    jsondb_upsert_with_schema(space, db, schema_rel, doc)
}

/// Liste des collections (résout `__cmd__jsondb_list_collections` dans main.rs)
#[tauri::command]
pub fn jsondb_list_collections(space: String, db: String) -> Result<Vec<String>, String> {
    let (_cfg, m) = mgr(&space, &db)?;
    // 💡 CORRECTION : Utilisation de list_collection_names
    m.list_collection_names().map_err(|e| e.to_string())
}
