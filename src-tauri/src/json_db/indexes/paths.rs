use crate::json_db::collections::collection::collection_root;
use crate::json_db::indexes::IndexType;
use crate::json_db::storage::JsonDbConfig;
use std::path::PathBuf;

/// Racine des index : {collection_root}/_indexes
pub fn indexes_root(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> PathBuf {
    // On réutilise la fonction officielle du module collections
    collection_root(cfg, space, db, collection).join("_indexes")
}

/// Chemin complet d'un index donné (hash, btree, ...) dans une collection.
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
        // CORRECTION : Ajout du cas manquant pour les index textuels
        IndexType::Text => "text.idx",
    };
    indexes_root(cfg, space, db, collection).join(format!("{index_name}.{extension}"))
}
