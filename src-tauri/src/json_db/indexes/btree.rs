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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_btree_sorting() {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path().join("s/d/collections/c/_indexes")).unwrap();

        let def = IndexDefinition {
            name: "age".into(),
            field_path: "/age".into(),
            index_type: IndexType::BTree,
            unique: false,
        };

        // Insert 30, then 10. BTree should order them 10, 30.
        update_btree_index(
            &cfg,
            "s",
            "d",
            "c",
            &def,
            "u1",
            None,
            Some(&json!({"age": 30})),
        )
        .unwrap();
        update_btree_index(
            &cfg,
            "s",
            "d",
            "c",
            &def,
            "u2",
            None,
            Some(&json!({"age": 10})),
        )
        .unwrap();

        let path = paths::index_path(&cfg, "s", "d", "c", "age", IndexType::BTree);
        let index: BTreeMap<String, Vec<String>> = driver::load(&path).unwrap();

        let keys: Vec<_> = index.keys().collect();
        assert_eq!(keys[0], "10");
        assert_eq!(keys[1], "30");
    }
}
