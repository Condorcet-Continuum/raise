// FICHIER : src-tauri/src/json_db/storage/file_storage.rs

use crate::json_db::storage::JsonDbConfig;

use crate::user_info; // Macro de log
use crate::utils::data::Value;
use crate::utils::error::{AppError, Result};
use crate::utils::io::Path;
use crate::utils::io::{self, include_dir, Dir};
use crate::utils::Utc;

// --- EMBARQUEMENT DES SCHÉMAS ---
static DEFAULT_SCHEMAS: Dir = include_dir!("$CARGO_MANIFEST_DIR/../schemas/v1");

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropMode {
    Soft,
    Hard,
}

pub async fn open_db(config: &JsonDbConfig, space: &str, db: &str) -> Result<()> {
    let db_path = config.db_root(space, db);
    if !io::exists(&db_path).await {
        return Err(AppError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Database does not exist: {:?}", db_path),
        )));
    }
    Ok(())
}

/// Crée l'arborescence physique ET déploie les schémas par défaut (Async).
pub async fn create_db(config: &JsonDbConfig, space: &str, db: &str) -> Result<()> {
    let db_root = config.db_root(space, db);
    io::create_dir_all(&db_root).await?;
    let schemas_dest = config.db_schemas_root(space, db).join("v1");

    if !io::exists(&schemas_dest).await {
        #[cfg(debug_assertions)]
        user_info!(
            "DB_INIT",
            "Déploiement des schémas standards dans {:?}",
            schemas_dest
        );
        io::ensure_dir(&schemas_dest).await?;

        // Extraction synchrone (CPU bound), acceptable ici car ponctuelle à l'init.
        DEFAULT_SCHEMAS
            .extract(&schemas_dest)
            .map_err(|e| AppError::Io(std::io::Error::other(e)))?;
    }

    Ok(())
}

pub async fn drop_db(config: &JsonDbConfig, space: &str, db: &str, mode: DropMode) -> Result<()> {
    let db_path = config.db_root(space, db);
    if !io::exists(&db_path).await {
        return Ok(());
    }

    match mode {
        DropMode::Hard => {
            io::remove_dir_all(&db_path).await?;
        }
        DropMode::Soft => {
            let timestamp = Utc::now().timestamp();
            let parent = db_path.parent().unwrap_or(&db_path);
            let new_name = format!("{}.deleted-{}", db, timestamp);
            let new_path = parent.join(new_name);

            io::rename(&db_path, &new_path).await?;
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
    io::create_dir_all(&col_path).await?;
    let file_path = col_path.join(format!("{}.json", id));
    io::write_json_atomic(&file_path, doc).await?;
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

    if !io::exists(&file_path).await {
        return Ok(None);
    }

    let doc: Value = io::read_json(&file_path).await?;
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

    if io::exists(&file_path).await {
        // Passage par référence (&file_path)
        io::remove_file(&file_path).await?;
    }
    Ok(())
}

pub async fn atomic_write<P: AsRef<Path>>(path: P, content: &[u8]) -> Result<()> {
    // Délégation totale à la façade utils::fs
    io::write_atomic(path.as_ref(), content).await?;
    Ok(())
}

/// Alias pour l'écriture binaire (utilisé par les index)
pub async fn atomic_write_binary<P: AsRef<Path>>(path: P, content: &[u8]) -> Result<()> {
    atomic_write(path, content).await
}

pub async fn save_database_index(path: &io::Path, data: &Value) -> Result<()> {
    // La primitive est maintenant ici, centralisée et testée
    io::write_json_compressed_atomic(path, data).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::{io::tempdir, json::json};
    #[tokio::test]
    async fn test_atomic_write() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test.txt");

        atomic_write(&file_path, b"Hello World").await.unwrap();
        assert!(file_path.exists());

        let content = io::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello World");
    }

    #[tokio::test]
    async fn test_document_lifecycle() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        let doc = json!({"name": "Refactor Test"});

        // Write
        write_document(&config, "s1", "d1", "c1", "doc1", &doc)
            .await
            .expect("Write failed");

        // Read
        let read = read_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .expect("Read failed")
            .expect("Doc not found");
        assert_eq!(read["name"], "Refactor Test");

        // Physical check via utils
        let path = config
            .db_collection_path("s1", "d1", "c1")
            .join("doc1.json");
        assert!(io::exists(&path).await);

        // Delete
        delete_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .expect("Delete failed");

        assert!(!io::exists(&path).await);
    }
}
