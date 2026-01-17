// FICHIER : src-tauri/src/json_db/collections/collection.rs

//! Primitives collections : gestion des dossiers et fichiers JSON d’une collection.
//! Pas de logique x_compute/validate ici — uniquement persistance et I/O.

use anyhow::{Context, Result};
use serde_json::Value;
use std::path::PathBuf;
use tokio::fs;

// On utilise atomic_write depuis file_storage (qui est maintenant async)
use crate::json_db::storage::file_storage::atomic_write;
use crate::json_db::storage::JsonDbConfig;

/// Racine des collections : {db_root}/collections/{collection}
pub fn collection_root(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> PathBuf {
    cfg.db_collection_path(space, db, collection)
}

/// Fichier d’un document : {collection_root}/{id}.json
fn doc_path(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str, id: &str) -> PathBuf {
    collection_root(cfg, space, db, collection).join(format!("{id}.json"))
}

/// S’assure que la collection existe (création récursive) - Async.
pub async fn create_collection_if_missing(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<()> {
    let root = collection_root(cfg, space, db, collection);
    if !root.exists() {
        fs::create_dir_all(&root)
            .await
            .with_context(|| format!("create_dir_all {}", root.display()))?;
    }
    Ok(())
}

/// Lit un document par son ID - Async.
pub async fn read_document(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<Value> {
    let path = doc_path(cfg, space, db, collection, id);
    let content = fs::read_to_string(&path)
        .await
        .with_context(|| format!("Document introuvable : {}/{}", collection, id))?;

    let doc: Value = serde_json::from_str(&content)
        .with_context(|| format!("JSON invalide : {}", path.display()))?;

    Ok(doc)
}

// --- FONCTIONS CRUD ---

pub async fn create_document(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    document: &Value,
) -> Result<()> {
    create_collection_if_missing(cfg, space, db, collection).await?;
    let path = doc_path(cfg, space, db, collection, id);
    let content = serde_json::to_string_pretty(document)?;
    atomic_write(path, content.as_bytes()).await?;
    Ok(())
}

pub async fn update_document(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    document: &Value,
) -> Result<()> {
    create_document(cfg, space, db, collection, id, document).await
}

pub async fn delete_document(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<()> {
    let path = doc_path(cfg, space, db, collection, id);
    if path.exists() {
        fs::remove_file(&path)
            .await
            .with_context(|| format!("Suppression {}", path.display()))?;
    }
    Ok(())
}

// --- AJOUT : Suppression de collection ---
pub async fn drop_collection(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<()> {
    let root = collection_root(cfg, space, db, collection);
    if root.exists() {
        fs::remove_dir_all(&root)
            .await
            .with_context(|| format!("Suppression collection {}", root.display()))?;
    }
    Ok(())
}

// --- FONCTIONS UTILITAIRES ---

pub async fn list_document_ids(
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

    let mut entries = fs::read_dir(&root).await?;
    while let Some(e) = entries.next_entry().await? {
        let p = e.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if !stem.starts_with('_') {
                    out.push(stem.to_string());
                }
            }
        }
    }
    out.sort();
    Ok(out)
}

pub async fn list_documents(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<Value>> {
    let ids = list_document_ids(cfg, space, db, collection).await?;
    let mut docs = Vec::with_capacity(ids.len());
    for id in ids {
        if let Ok(doc) = read_document(cfg, space, db, collection, &id).await {
            docs.push(doc);
        }
    }
    Ok(docs)
}

pub async fn list_collection_names_fs(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
) -> Result<Vec<String>> {
    let root = cfg.db_root(space, db).join("collections");
    let mut out = Vec::new();
    if !root.exists() {
        return Ok(out);
    }
    let mut entries = fs::read_dir(root).await?;
    while let Some(e) = entries.next_entry().await? {
        let ty = e.file_type().await?;
        if ty.is_dir() {
            if let Ok(name) = e.file_name().into_string() {
                out.push(name);
            }
        }
    }
    out.sort();
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_collection_crud_async() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let (s, d, c) = ("space", "db", "col");

        let doc = json!({"id": "1", "data": "test"});

        // Create
        create_document(&config, s, d, c, "1", &doc).await.unwrap();

        // Read
        let read = read_document(&config, s, d, c, "1").await.unwrap();
        assert_eq!(read["data"], "test");

        // List
        let ids = list_document_ids(&config, s, d, c).await.unwrap();
        assert_eq!(ids, vec!["1"]);

        // Delete
        delete_document(&config, s, d, c, "1").await.unwrap();
        let ids_after = list_document_ids(&config, s, d, c).await.unwrap();
        assert!(ids_after.is_empty());
    }
}
