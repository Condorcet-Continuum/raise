// FICHIER : src-tauri/src/json_db/storage/file_storage.rs

use crate::json_db::storage::JsonDbConfig;
use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tokio::io::AsyncWriteExt;

// --- EMBARQUEMENT DES SCHÉMAS ---
static DEFAULT_SCHEMAS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../schemas/v1");

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropMode {
    Soft,
    Hard,
}

pub fn open_db(config: &JsonDbConfig, space: &str, db: &str) -> Result<()> {
    let db_path = config.db_root(space, db);
    if !db_path.exists() {
        return Err(anyhow::anyhow!("Database does not exist: {:?}", db_path));
    }
    Ok(())
}

/// Crée l'arborescence physique ET déploie les schémas par défaut (Async).
pub async fn create_db(config: &JsonDbConfig, space: &str, db: &str) -> Result<()> {
    let db_root = config.db_root(space, db);

    if !db_root.exists() {
        fs::create_dir_all(&db_root)
            .await
            .context("Failed to create DB root directory")?;
    }

    let schemas_dest = config.db_schemas_root(space, db).join("v1");

    if !schemas_dest.exists() {
        #[cfg(debug_assertions)]
        println!(
            "📦 Déploiement des schémas standards dans {:?}",
            schemas_dest
        );

        fs::create_dir_all(&schemas_dest).await?;

        // Extraction synchrone (CPU bound), acceptable ici car ponctuelle à l'init.
        DEFAULT_SCHEMAS
            .extract(&schemas_dest)
            .context("Failed to extract embedded schemas")?;
    }

    Ok(())
}

pub async fn drop_db(config: &JsonDbConfig, space: &str, db: &str, mode: DropMode) -> Result<()> {
    let db_path = config.db_root(space, db);
    if !db_path.exists() {
        return Ok(());
    }

    match mode {
        DropMode::Hard => {
            fs::remove_dir_all(&db_path)
                .await
                .with_context(|| format!("Failed to remove DB {:?}", db_path))?;
        }
        DropMode::Soft => {
            let timestamp = chrono::Utc::now().timestamp();
            let parent = db_path.parent().unwrap();
            let new_name = format!("{}.deleted-{}", db, timestamp);
            let new_path = parent.join(new_name);
            fs::rename(&db_path, &new_path)
                .await
                .with_context(|| "Failed to soft drop DB")?;
        }
    }
    Ok(())
}

pub async fn write_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    doc: &Value,
) -> Result<()> {
    let col_path = config.db_collection_path(space, db, collection);
    if !col_path.exists() {
        fs::create_dir_all(&col_path).await?;
    }
    let file_path = col_path.join(format!("{}.json", id));
    let content = serde_json::to_string_pretty(doc)?;
    atomic_write(file_path, content.as_bytes()).await?;
    Ok(())
}

pub async fn read_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<Option<Value>> {
    let file_path = config
        .db_collection_path(space, db, collection)
        .join(format!("{}.json", id));

    if !file_path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(file_path).await?;
    let doc = serde_json::from_str(&content)?;
    Ok(Some(doc))
}

pub async fn delete_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> Result<()> {
    let file_path = config
        .db_collection_path(space, db, collection)
        .join(format!("{}.json", id));

    if file_path.exists() {
        fs::remove_file(file_path).await?;
    }
    Ok(())
}

/// Écriture atomique sécurisée (write -> sync -> rename)
pub async fn atomic_write<P: AsRef<Path>>(path: P, content: &[u8]) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).await?;
        }
    }

    let temp_path = path.with_extension("tmp");

    {
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(content).await?;
        // On force l'écriture physique sur le plateau du disque
        file.sync_all().await?;
    }

    fs::rename(&temp_path, path).await?;
    Ok(())
}

pub async fn atomic_write_binary<P: AsRef<Path>>(path: P, content: &[u8]) -> Result<()> {
    atomic_write(path, content).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_atomic_write() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        atomic_write(&file_path, b"Hello World").await.unwrap();
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello World");
    }

    #[tokio::test]
    async fn test_document_lifecycle() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        let doc = json!({"name": "Test"});

        // Write
        write_document(&config, "s1", "d1", "c1", "doc1", &doc)
            .await
            .unwrap();

        // Read
        let read = read_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read["name"], "Test");

        // Delete
        delete_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .unwrap();
        let deleted = read_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .unwrap();
        assert!(deleted.is_none());
    }
}
