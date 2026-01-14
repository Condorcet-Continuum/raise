// FICHIER : src-tauri/src/json_db/indexes/hash.rs

use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

#[allow(clippy::too_many_arguments)]
pub fn update_hash_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> Result<()> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);
    driver::update::<HashMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_hash_update() {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        // Création structure dossiers
        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        std::fs::create_dir_all(&idx_dir).unwrap();

        let def = IndexDefinition {
            name: "email".into(),
            field_path: "/email".into(),
            index_type: IndexType::Hash,
            unique: true,
        };

        let doc = json!({ "email": "test@mail.com" });

        // Insertion
        update_hash_index(&cfg, "s", "d", "c", &def, "1", None, Some(&doc)).unwrap();

        // Vérification lecture
        let path = paths::index_path(&cfg, "s", "d", "c", "email", IndexType::Hash);
        let index: HashMap<String, Vec<String>> = driver::load(&path).unwrap();
        assert!(index.contains_key("\"test@mail.com\"")); // Note: les clés sont stringifiées par serde_json
    }
}
