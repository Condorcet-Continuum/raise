//! Primitives collections : gestion des dossiers et fichiers JSON d’une collection.
//! Pas de logique x_compute/validate ici — uniquement persistance et I/O.

use anyhow::{anyhow, Context, Result};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

use crate::json_db::storage::file_storage::atomic_write_json;
use crate::json_db::storage::JsonDbConfig;

/// Racine DB à partir de la racine des schémas (…/schemas/v1 → ..)
fn db_root(cfg: &JsonDbConfig, space: &str, db: &str) -> PathBuf {
    let schemas = cfg.db_schemas_root(space, db);
    schemas
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| Path::new(".").to_path_buf())
}

/// Racine des collections : {db_root}/collections/{collection}
pub fn collection_root(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> PathBuf {
    db_root(cfg, space, db).join("collections").join(collection)
}

/// Fichier d’un document : {collection_root}/{id}.json
fn doc_path(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str, id: &str) -> PathBuf {
    collection_root(cfg, space, db, collection).join(format!("{id}.json"))
}

/// S’assure que la collection existe (création récursive).
pub fn create_collection_if_missing(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<()> {
    let root = collection_root(cfg, space, db, collection);
    fs::create_dir_all(&root).with_context(|| format!("create_dir_all {}", root.display()))?;
    Ok(())
}

/// Supprime une collection (danger : efface le dossier).
pub fn drop_collection(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> Result<()> {
    let root = collection_root(cfg, space, db, collection);
    if root.exists() {
        fs::remove_dir_all(&root).with_context(|| format!("remove_dir_all {}", root.display()))?;
    }
    Ok(())
}

/// Extrait l’id d’un document (doit être une string non vide).
fn extract_id(doc: &Value) -> Result<String> {
    let id = doc
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow!("document missing string field 'id'"))?;
    if id.is_empty() {
        return Err(anyhow!("document 'id' is empty"));
    }
    Ok(id.to_string())
}

/// Insert (échec si l’id existe déjà).
pub fn persist_insert(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc: &Value,
) -> Result<()> {
    let id = extract_id(doc)?;
    let path = doc_path(cfg, space, db, collection, &id);
    if path.exists() {
        return Err(anyhow!("document with id '{}' already exists", id));
    }
    // Appel à la version partagée
    atomic_write_json(&path, doc)?;
    Ok(())
}

/// Update (échec si l’id n’existe pas) — remplacement complet du document.
pub fn persist_update(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc: &Value,
) -> Result<()> {
    let id = extract_id(doc)?;
    let path = doc_path(cfg, space, db, collection, &id);
    if !path.exists() {
        return Err(anyhow!("document with id '{}' not found", id));
    }
    // Appel à la version partagée
    atomic_write_json(&path, doc)?;
    Ok(())
}

/// Get par id.
pub fn read_document(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<Value> {
    let path = doc_path(cfg, space, db, collection, id);
    let data = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let json: Value =
        serde_json::from_str(&data).with_context(|| format!("parse json {}", path.display()))?;
    Ok(json)
}

/// Delete par id (idempotent : OK si déjà absent).
pub fn delete_document(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<()> {
    let path = doc_path(cfg, space, db, collection, id);
    if path.exists() {
        fs::remove_file(&path).with_context(|| format!("remove_file {}", path.display()))?;
    }
    Ok(())
}

/// Liste les ids (fichiers *.json).
pub fn list_document_ids(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<String>> {
    let root = collection_root(cfg, space, db, collection);
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    for e in fs::read_dir(&root).with_context(|| root.display().to_string())? {
        let e = e?;
        let p = e.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                out.push(stem.to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Liste et charge tous les documents (attention perfs).
pub fn list_documents(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<Value>> {
    let ids = list_document_ids(cfg, space, db, collection)?;
    let mut docs = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(doc) = read_document(cfg, space, db, collection, &id) {
            docs.push(doc);
        }
    }
    Ok(docs)
}

/// Liste tous les noms de collections (dossiers) existantes pour la DB.
pub fn list_collection_names_fs(cfg: &JsonDbConfig, space: &str, db: &str) -> Result<Vec<String>> {
    let root = db_root(cfg, space, db).join("collections");
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }

    // Lire les dossiers
    for e in fs::read_dir(&root).with_context(|| root.display().to_string())? {
        let e = e?;
        let p = e.path();
        if p.is_dir() {
            // On ne veut que les dossiers
            if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    Ok(out)
}
