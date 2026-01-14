// FICHIER : src-tauri/src/json_db/storage/file_storage.rs

use crate::json_db::storage::JsonDbConfig;
use anyhow::{Context, Result};
use include_dir::{include_dir, Dir};
use serde_json::Value;
use std::fs;
use std::path::Path;

// --- EMBARQUEMENT DES SCHÉMAS ---
// Chemin relatif au Cargo.toml de src-tauri
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

/// Crée l'arborescence physique ET déploie les schémas par défaut.
pub fn create_db(config: &JsonDbConfig, space: &str, db: &str) -> Result<()> {
    let db_root = config.db_root(space, db);

    // 1. Création des dossiers de base
    if !db_root.exists() {
        fs::create_dir_all(&db_root).context("Failed to create DB root directory")?;
    }

    // 2. Déploiement automatique des schémas embarqués
    let schemas_dest = config.db_schemas_root(space, db).join("v1");

    if !schemas_dest.exists() {
        #[cfg(debug_assertions)]
        println!(
            "📦 Déploiement des schémas standards dans {:?}",
            schemas_dest
        );

        fs::create_dir_all(&schemas_dest)?;
        DEFAULT_SCHEMAS
            .extract(&schemas_dest)
            .context("Failed to extract embedded schemas")?;
    }

    Ok(())
}

pub fn drop_db(config: &JsonDbConfig, space: &str, db: &str, mode: DropMode) -> Result<()> {
    let db_path = config.db_root(space, db);
    if !db_path.exists() {
        return Ok(());
    }

    match mode {
        DropMode::Hard => {
            fs::remove_dir_all(&db_path)
                .with_context(|| format!("Failed to remove DB {:?}", db_path))?;
        }
        DropMode::Soft => {
            let timestamp = chrono::Utc::now().timestamp();
            let parent = db_path.parent().unwrap();
            let new_name = format!("{}.deleted-{}", db, timestamp);
            let new_path = parent.join(new_name);
            fs::rename(&db_path, &new_path).with_context(|| "Failed to soft drop DB")?;
        }
    }
    Ok(())
}

pub fn write_document(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    doc: &Value,
) -> Result<()> {
    let col_path = config.db_collection_path(space, db, collection);
    if !col_path.exists() {
        fs::create_dir_all(&col_path)?;
    }
    let file_path = col_path.join(format!("{}.json", id));
    let content = serde_json::to_string_pretty(doc)?;
    atomic_write(file_path, content)?;
    Ok(())
}

pub fn read_document(
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
    let content = fs::read_to_string(file_path)?;
    let doc = serde_json::from_str(&content)?;
    Ok(Some(doc))
}

pub fn delete_document(
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
        fs::remove_file(file_path)?;
    }
    Ok(())
}

/// Écriture atomique (write -> sync -> rename)
pub fn atomic_write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, content: C) -> Result<()> {
    let path = path.as_ref();
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent)?;
        }
    }
    // Fichier temporaire dans le même dossier
    let temp_path = path.with_extension("tmp");
    fs::write(&temp_path, content)?;
    fs::rename(&temp_path, path)?;
    Ok(())
}

pub fn atomic_write_binary<P: AsRef<Path>>(path: P, content: &[u8]) -> Result<()> {
    atomic_write(path, content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_atomic_write() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        atomic_write(&file_path, "Hello World").unwrap();
        assert!(file_path.exists());

        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello World");
    }

    #[test]
    fn test_document_lifecycle() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        let doc = json!({"name": "Test"});

        // Write
        write_document(&config, "s1", "d1", "c1", "doc1", &doc).unwrap();

        // Read
        let read = read_document(&config, "s1", "d1", "c1", "doc1")
            .unwrap()
            .unwrap();
        assert_eq!(read["name"], "Test");

        // Delete
        delete_document(&config, "s1", "d1", "c1", "doc1").unwrap();
        let deleted = read_document(&config, "s1", "d1", "c1", "doc1").unwrap();
        assert!(deleted.is_none());
    }
}
