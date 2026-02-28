// FICHIER : src-tauri/src/json_db/storage/file_storage.rs

use crate::json_db::storage::JsonDbConfig;
use crate::utils::config::AppConfig;
use crate::utils::io::{self, Path};
use crate::utils::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DropMode {
    Soft,
    Hard,
}

pub async fn open_db(config: &JsonDbConfig, space: &str, db: &str) -> RaiseResult<()> {
    let db_path = config.db_root(space, db);

    if !io::exists(&db_path).await {
        raise_error!(
            "ERR_DB_FS_NOT_FOUND",
            error = format!(
                "Le répertoire de la base de données est introuvable : {}",
                db
            ),
            context = json!({
                "space": space,
                "db_name": db,
                "resolved_path": db_path,
                "action": "open_database_storage",
                "hint": "Si c'est un premier lancement, assurez-vous d'appeler 'create_db' avant 'open_db'."
            })
        );
    }

    Ok(())
}

/// Crée l'arborescence physique de la base de données.
/// Note : Architecture "Zéro Copie", les schémas ne sont plus copiés ici.
pub async fn create_db(config: &JsonDbConfig, space: &str, db: &str) -> RaiseResult<bool> {
    let db_root = config.db_root(space, db);

    if io::exists(&db_root).await {
        return Ok(false);
    }

    // Création simple du dossier racine
    io::create_dir_all(&db_root).await?;

    // Vérification : Est-ce qu'on crée la base système ?
    let app_config = AppConfig::get();
    // ✅ CORRECTION : Utilisation des nouveaux champs 'system_domain' / 'system_db'
    let sys_domain = &app_config.system_domain;
    let sys_db = &app_config.system_db;

    if space == sys_domain && db == sys_db {
        #[cfg(debug_assertions)]
        println!("🚀 Initialisation de la base SYSTEME détectée.");
    }

    Ok(true)
}

pub async fn drop_db(
    config: &JsonDbConfig,
    space: &str,
    db: &str,
    mode: DropMode,
) -> RaiseResult<()> {
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
) -> RaiseResult<()> {
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
) -> RaiseResult<Option<Value>> {
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
) -> RaiseResult<()> {
    let file_path = config
        .db_collection_path(space, db, collection)
        .join(format!("{}.json", id));

    if io::exists(&file_path).await {
        io::remove_file(&file_path).await?;
    }
    Ok(())
}

pub async fn atomic_write<P: AsRef<Path>>(path: P, content: &[u8]) -> RaiseResult<()> {
    io::write_atomic(path.as_ref(), content).await?;
    Ok(())
}

pub async fn atomic_write_binary<P: AsRef<Path>>(path: P, content: &[u8]) -> RaiseResult<()> {
    atomic_write(path, content).await
}

pub async fn save_database_index(path: &io::Path, data: &Value) -> RaiseResult<()> {
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

        write_document(&config, "s1", "d1", "c1", "doc1", &doc)
            .await
            .expect("Write failed");

        let read = read_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .expect("Read failed")
            .expect("Doc not found");
        assert_eq!(read["name"], "Refactor Test");

        let path = config
            .db_collection_path("s1", "d1", "c1")
            .join("doc1.json");
        assert!(io::exists(&path).await);

        delete_document(&config, "s1", "d1", "c1", "doc1")
            .await
            .expect("Delete failed");

        assert!(!io::exists(&path).await);
    }
}
