// FICHIER : src-tauri/src/json_db/collections/collection.rs

//! Primitives collections : gestion des dossiers et fichiers JSON d’une collection.
//! Pas de logique x_compute/validate ici — uniquement persistance et I/O.

use crate::utils::io::{self, PathBuf};
use crate::utils::prelude::*;

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
    io::ensure_dir(&root).await?;
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
    let doc = io::read_json(&path).await?;
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
    io::write_json_atomic(&path, document).await?;
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
    io::remove_file(&path).await?;
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
    io::remove_dir_all(&root).await?;
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
    if !io::exists(&root).await {
        return Ok(out);
    }
    let mut entries = io::read_dir(&root).await?;
    while let Some(e) = entries
        .next_entry()
        .await
        .map_err(crate::utils::AppError::Io)?
    {
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
    if !io::exists(&root).await {
        return Ok(out);
    }
    let mut entries = io::read_dir(&root).await?;
    while let Some(e) = entries
        .next_entry()
        .await
        .map_err(crate::utils::AppError::Io)?
    {
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
    use crate::utils::{io::tempdir, json::json};
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
