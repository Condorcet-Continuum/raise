//! Façade Collections : API haut niveau (stable) pour manipuler les documents
//! via un schéma JSON interne (x_compute → validate), puis persister en FS.
//!
//! Règle : toujours passer par ces fonctions depuis le reste du projet (CLI, UI Tauri).

pub mod collection;
pub mod manager;

use anyhow::Result;
use serde_json::Value;
use std::path::Path;

use crate::json_db::{
    schema::{SchemaRegistry, SchemaValidator},
    storage::JsonDbConfig,
};

/// Déduit le nom logique de la collection à partir du chemin de schéma relatif,
/// ex. "actors/actor.schema.json" → "actors".
fn collection_from_schema_rel(schema_rel: &str) -> String {
    schema_rel
        .split('/')
        .next()
        .unwrap_or("default")
        .to_string()
}

/// Initialise une DB (si besoin) et crée la collection si manquante.
pub fn create_collection(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<()> {
    collection::create_collection_if_missing(cfg, space, db, collection)
}

/// Supprime (drop) une collection (efface les fichiers).
pub fn drop_collection(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> Result<()> {
    collection::drop_collection(cfg, space, db, collection)
}

/// Insert avec schéma :
/// - compile le schéma (depuis la DB)
/// - x_compute + validate (préremplit $schema, id, createdAt, updatedAt si manquants)
/// - persiste dans la collection déduite du schéma
pub fn insert_with_schema(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    schema_rel: &str, // ex: "actors/actor.schema.json"
    mut doc: Value,
) -> Result<Value> {
    // 1) Registre/validator
    let reg = SchemaRegistry::from_db(cfg, space, db)?;
    let root_uri = reg.uri(schema_rel);
    let validator = SchemaValidator::compile_with_registry(&root_uri, &reg)?;

    // 2) x_compute + validate (force $schema si absent)
    validator.compute_then_validate(&mut doc)?;

    // 3) Collection cible
    let collection = collection_from_schema_rel(schema_rel);
    collection::create_collection_if_missing(cfg, space, db, &collection)?;
    collection::persist_insert(cfg, space, db, &collection, &doc)?;

    Ok(doc)
}

/// Insert direct (sans schéma) — à n’utiliser que si le document est déjà conforme.
/// (Pour rester cohérent, privilégier `insert_with_schema`.)
pub fn insert_raw(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc: &Value,
) -> Result<()> {
    collection::create_collection_if_missing(cfg, space, db, collection)?;
    collection::persist_insert(cfg, space, db, collection, doc)
}

/// Update avec schéma : recompute + validate + persist (remplace le doc par id).
pub fn update_with_schema(
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

    let collection = collection_from_schema_rel(schema_rel);
    collection::persist_update(cfg, space, db, &collection, &doc)?;
    Ok(doc)
}

/// Update direct (sans schéma) — remplace intégralement par id.
pub fn update_raw(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc: &Value,
) -> Result<()> {
    collection::persist_update(cfg, space, db, collection, doc)
}

/// Récupère un document par id.
pub fn get(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str, id: &str) -> Result<Value> {
    collection::read_document(cfg, space, db, collection, id)
}

/// Supprime un document par id.
pub fn delete(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str, id: &str) -> Result<()> {
    collection::delete_document(cfg, space, db, collection, id)
}

/// Liste les documents (ids) d’une collection.
pub fn list_ids(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<String>> {
    collection::list_document_ids(cfg, space, db, collection)
}

/// Liste et charge les documents (attention aux perfs si volumineux).
pub fn list_all(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> Result<Vec<Value>> {
    collection::list_documents(cfg, space, db, collection)
}

/// Utilitaire pour tests : retourne la racine DB (FS)
pub fn db_root_path(cfg: &JsonDbConfig, space: &str, db: &str) -> std::path::PathBuf {
    // On se base sur db_schemas_root(...)/.. = racine DB
    let schemas = cfg.db_schemas_root(space, db);
    schemas
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}
