// FICHIER : src-tauri/src/json_db/collections/collection.rs

//! Primitives collections : gestion des dossiers et fichiers JSON d’une collection.
//! 🚀 V2 : Refactorisé pour s'interfacer avec le StorageEngine (et bénéficier du Cache LRU).

use crate::utils::prelude::*;

use crate::json_db::storage::{JsonDbConfig, StorageEngine};

/// Racine des collections : {db_root}/collections/{collection}
pub fn collection_root(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> PathBuf {
    cfg.db_collection_path(space, db, collection)
}

/// S’assure que la collection existe (création récursive) - Async.
pub async fn create_collection_if_missing(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> RaiseResult<()> {
    let root = collection_root(cfg, space, db, collection);
    fs::ensure_dir_async(&root).await?;
    Ok(())
}

// --- FONCTIONS CRUD (Déléguées au StorageEngine pour utiliser le cache) ---

/// Lit un document par son ID via le StorageEngine.
pub async fn read_document(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> RaiseResult<JsonValue> {
    // On tape dans le cache/disque via le StorageEngine
    let doc_opt = storage.read_document(space, db, collection, id).await?;

    match doc_opt {
        Some(doc) => Ok(doc),
        None => raise_error!(
            "ERR_DB_DOCUMENT_NOT_FOUND",
            error = format!(
                "Document '{}' introuvable dans la collection '{}'",
                id, collection
            ),
            context = json_value!({
                "space": space,
                "db": db,
                "collection": collection,
                "_id": id,
                "action": "read_document_with_cache"
            })
        ),
    }
}

pub async fn create_document(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    document: &JsonValue,
) -> RaiseResult<()> {
    create_collection_if_missing(&storage.config, space, db, collection).await?;
    storage
        .write_document(space, db, collection, id, document)
        .await?;
    Ok(())
}

pub async fn update_document(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
    document: &JsonValue,
) -> RaiseResult<()> {
    // La logique d'écriture est identique pour une création ou une mise à jour au niveau I/O
    create_document(storage, space, db, collection, id, document).await
}

pub async fn delete_document(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    id: &str,
) -> RaiseResult<()> {
    storage.delete_document(space, db, collection, id).await?;
    Ok(())
}

// --- GESTION DES DOSSIERS (I/O pur, pas de cache) ---

pub async fn drop_collection(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> RaiseResult<()> {
    let root = collection_root(cfg, space, db, collection);
    fs::remove_dir_all_async(&root).await?;
    Ok(())
}

pub async fn list_document_ids(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    limit: Option<usize>,  // 🎯 NOUVEAU
    offset: Option<usize>, // 🎯 NOUVEAU
) -> RaiseResult<Vec<String>> {
    let root = collection_root(cfg, space, db, collection);
    let mut out = Vec::new();
    if !fs::exists_async(&root).await {
        return Ok(out);
    }

    let mut entries = fs::read_dir_async(&root).await?;
    while let Some(e) = match entries.next_entry().await {
        Ok(entry) => entry,
        Err(err) => raise_error!("ERR_FS_READ_DIR_ENTRY", error = err),
    } {
        let p = e.path();
        if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("json") {
            if let Some(stem) = p.file_stem().and_then(|s| s.to_str()) {
                if !stem.starts_with('_') {
                    out.push(stem.to_string());
                }
            }
        }
    }

    // On trie d'abord pour garantir un ordre déterministe
    out.sort();

    // 🎯 FIX : Application de la pagination sur les IDs (très léger en RAM)
    let skip_val = offset.unwrap_or(0);
    let take_val = limit.unwrap_or(out.len());

    let paginated_ids = out.into_iter().skip(skip_val).take(take_val).collect();

    Ok(paginated_ids)
}

pub async fn list_documents(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    limit: Option<usize>,
    offset: Option<usize>,
) -> RaiseResult<Vec<JsonValue>> {
    // On récupère uniquement la sous-liste des IDs demandés
    let ids = list_document_ids(&storage.config, space, db, collection, limit, offset).await?;
    let mut docs = Vec::with_capacity(ids.len());

    for id in ids {
        // La boucle ne tournera que `limit` fois maximum ! La RAM est sauvée.
        if let Ok(doc) = read_document(storage, space, db, collection, &id).await {
            docs.push(doc);
        }
    }
    Ok(docs)
}

pub async fn list_collection_names_fs(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
) -> RaiseResult<Vec<String>> {
    let root = cfg.db_root(space, db).join("collections");
    let mut out = Vec::new();
    if !fs::exists_async(&root).await {
        return Ok(out);
    }
    let mut entries = fs::read_dir_async(&root).await?;
    while let Some(e) = match entries.next_entry().await {
        Ok(entry) => entry,
        Err(err) => raise_error!(
            "ERR_FS_ITERATION_FAIL",
            error = err,
            context = json_value!({ "root": root, "action": "list_next_entry" })
        ),
    } {
        let ty = match e.file_type().await {
            Ok(t) => t,
            Err(err) => raise_error!(
                "ERR_FS_METADATA_FAIL",
                error = err,
                context = json_value!({ "path": e.path(), "action": "get_file_type" })
            ),
        };

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

    #[async_test]
    async fn test_collection_crud_async_with_storage() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());

        // 1. Initialisation du StorageEngine pour les tests
        let storage = StorageEngine::new(config);
        let (s, d, c) = ("space", "db", "col");

        let doc = json_value!({"id": "1", "data": "test"});

        // Create
        create_document(&storage, s, d, c, "1", &doc).await.unwrap();

        // Read
        let read = read_document(&storage, s, d, c, "1").await.unwrap();
        assert_eq!(read["data"], "test");

        // List
        let ids = list_document_ids(&storage.config, s, d, c, None, None)
            .await
            .unwrap();
        assert_eq!(ids, vec!["1"]);

        // Delete
        delete_document(&storage, s, d, c, "1").await.unwrap();
        let ids_after = list_document_ids(&storage.config, s, d, c, None, None)
            .await
            .unwrap();
        assert!(ids_after.is_empty());
    }
}
