// FICHIER : src-tauri/src/json_db/indexes/paths.rs

use crate::json_db::collections::collection::collection_root;
use crate::json_db::indexes::IndexType;
use crate::json_db::storage::JsonDbConfig;
use std::path::PathBuf;

/// Racine des index : {collection_root}/_indexes
pub fn indexes_root(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> PathBuf {
    collection_root(cfg, space, db, collection).join("_indexes")
}

/// Chemin complet d'un index donné
pub fn index_path(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    index_name: &str,
    index_type: IndexType,
) -> PathBuf {
    let extension = match index_type {
        IndexType::Hash => "hash.idx",
        IndexType::BTree => "btree.idx",
        IndexType::Text => "text.idx",
    };
    indexes_root(cfg, space, db, collection).join(format!("{index_name}.{extension}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_paths_structure() {
        let cfg = JsonDbConfig::new(PathBuf::from("/data"));
        let path = index_path(&cfg, "space", "db", "users", "email", IndexType::Hash);

        // Vérifie la structure standard : /data/space/db/collections/users/_indexes/email.hash.idx
        let s = path.to_string_lossy();
        assert!(s.contains("collections"));
        assert!(s.contains("users"));
        assert!(s.contains("_indexes"));
        assert!(s.ends_with("email.hash.idx"));
    }
}
