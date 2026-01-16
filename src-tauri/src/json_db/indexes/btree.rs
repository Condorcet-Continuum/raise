// FICHIER : src-tauri/src/json_db/indexes/btree.rs

use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

#[allow(clippy::too_many_arguments)]
pub fn update_btree_index(
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
    driver::update::<BTreeMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc)
}

/// Recherche exacte via l'index BTree.
/// Note: La structure BTree permettra à l'avenir des recherches par plage (Range Search)
/// mais pour l'instant nous exposons une recherche exacte standard.
pub fn search_btree_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    value: &Value,
) -> Result<Vec<String>> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);
    let key = value.to_string();
    driver::search::<BTreeMap<String, Vec<String>>>(&path, &key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use serde_json::json;
    use tempfile::tempdir;

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, cfg)
    }

    #[test]
    fn test_btree_lifecycle() {
        let (dir, cfg) = setup_env();
        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        std::fs::create_dir_all(&idx_dir).unwrap();

        let def = IndexDefinition {
            name: "age".into(),
            field_path: "/age".into(),
            index_type: IndexType::BTree,
            unique: false,
        };

        // 1. Insertion
        let doc1 = json!({ "age": 30 });
        update_btree_index(&cfg, "s", "d", "c", &def, "u1", None, Some(&doc1)).unwrap();

        let doc2 = json!({ "age": 25 });
        update_btree_index(&cfg, "s", "d", "c", &def, "u2", None, Some(&doc2)).unwrap();

        // 2. Recherche Exacte
        let results = search_btree_index(&cfg, "s", "d", "c", &def, &json!(30)).unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "u1");

        let results_25 = search_btree_index(&cfg, "s", "d", "c", &def, &json!(25)).unwrap();
        assert_eq!(results_25.len(), 1);
        assert_eq!(results_25[0], "u2");

        // 3. Recherche vide
        let results_empty = search_btree_index(&cfg, "s", "d", "c", &def, &json!(99)).unwrap();
        assert!(results_empty.is_empty());
    }
}
