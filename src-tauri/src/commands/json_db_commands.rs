//! JSON-DB Tauri commands
//!
//! Ces commandes exposent les opérations principales (CRUD) via Tauri.

use serde_json::Value;
use std::path::Path;

// 2. QueryInput est dans json_db::query
use crate::json_db::query::{QueryEngine, QueryInput, QueryResult};
use crate::json_db::transactions::TransactionManager;

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
    schema: Option<String>, // <--- 1. Ajout du paramètre dans la commande Tauri
) -> Result<(), String> {
    let (_cfg, m) = mgr(&space, &db)?;

    // <--- 2. Passage du paramètre au manager (qui attend maintenant 2 arguments)
    m.create_collection(&collection, schema)
        .map_err(|e| e.to_string())
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

/// Structure d'entrée pour une transaction depuis le frontend
#[derive(serde::Deserialize)]
pub struct TransactionRequest {
    pub operations: Vec<OperationRequest>,
}

#[derive(serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum OperationRequest {
    Insert { collection: String, doc: Value },
    Update { collection: String, doc: Value },
    Delete { collection: String, id: String },
}

/// Exécute une transaction atomique (ACID)
#[tauri::command]
pub fn jsondb_execute_transaction(
    space: String,
    db: String,
    request: TransactionRequest,
) -> Result<(), String> {
    // 1. Init Config & Manager
    let cfg = cfg_from_repo_env()?;

    // On s'assure que la DB existe
    if crate::json_db::storage::file_storage::open_db(&cfg, &space, &db).is_err() {
        return Err(format!("Database {}/{} does not exist", space, db));
    }

    let tm = TransactionManager::new(&cfg, &space, &db);

    // 2. Exécution transactionnelle
    tm.execute(|tx| {
        for op in request.operations {
            match op {
                OperationRequest::Insert {
                    collection,
                    mut doc,
                } => {
                    // CORRECTION : On extrait l'ID et on le transforme immédiatement en String (owned)
                    // Cela libère l'emprunt sur `doc` avant de le modifier/déplacer.
                    let id = match doc.get("id").and_then(|v| v.as_str()) {
                        Some(s) => s.to_string(),
                        None => uuid::Uuid::new_v4().to_string(),
                    };

                    // Maintenant que l'emprunt est fini, on peut muter `doc`
                    if let Some(obj) = doc.as_object_mut() {
                        obj.insert("id".to_string(), serde_json::Value::String(id.clone()));
                    }

                    // Et on peut déplacer `doc` sans erreur
                    tx.add_insert(&collection, &id, doc);
                }
                OperationRequest::Update { collection, doc } => {
                    // CORRECTION : Même problème ici, `id` ne doit pas être une référence (&str)
                    // car `doc` est déplacé (move) dans `add_update` juste après.
                    let id = doc
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string()) // On clone en String ici
                        .ok_or_else(|| anyhow::anyhow!("Missing id for update"))?;

                    // On passe `None` pour old_doc pour l'instant (TODO: Rollback)
                    tx.add_update(&collection, &id, None, doc);
                }
                OperationRequest::Delete { collection, id } => {
                    tx.add_delete(&collection, &id, None);
                }
            }
        }
        Ok(())
    })
    .map_err(|e| e.to_string())?;

    Ok(())
}
