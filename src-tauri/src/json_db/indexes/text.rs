// FICHIER : src-tauri/src/json_db/indexes/text.rs

use anyhow::Result;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::driver;
use super::{paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

/// Tokenizer simple : minuscules, alphanumérique seulement
fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn update_text_index(
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

    let mut index: HashMap<String, Vec<String>> = driver::load(&path)?;
    let mut changed = false;

    // Suppression anciens tokens
    if let Some(doc) = old_doc {
        if let Some(val) = doc.pointer(&def.field_path).and_then(|v| v.as_str()) {
            for token in tokenize(val) {
                if let Some(ids) = index.get_mut(&token) {
                    if let Some(pos) = ids.iter().position(|x| x == doc_id) {
                        ids.swap_remove(pos);
                        changed = true;
                    }
                }
                // Cleanup
                if index.get(&token).is_some_and(|ids| ids.is_empty()) {
                    index.remove(&token);
                }
            }
        }
    }

    // Ajout nouveaux tokens
    if let Some(doc) = new_doc {
        if let Some(val) = doc.pointer(&def.field_path).and_then(|v| v.as_str()) {
            for token in tokenize(val) {
                let ids = index.entry(token).or_default();
                if !ids.contains(&doc_id.to_string()) {
                    ids.push(doc_id.to_string());
                    changed = true;
                }
            }
        }
    }

    if changed {
        driver::save(&path, &index)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use serde_json::json;
    use tempfile::tempdir;

    #[test]
    fn test_text_tokenization() {
        let tokens = tokenize("Hello, World! 123");
        assert!(tokens.contains("hello"));
        assert!(tokens.contains("world"));
        assert!(tokens.contains("123"));
        assert!(!tokens.contains(","));
    }

    #[test]
    fn test_text_index_update() {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path().join("s/d/collections/c/_indexes")).unwrap();

        let def = IndexDefinition {
            name: "bio".into(),
            field_path: "/bio".into(),
            index_type: IndexType::Text,
            unique: false,
        };

        // Insert doc
        update_text_index(
            &cfg,
            "s",
            "d",
            "c",
            &def,
            "1",
            None,
            Some(&json!({"bio": "Rust dev"})),
        )
        .unwrap();

        let path = paths::index_path(&cfg, "s", "d", "c", "bio", IndexType::Text);
        let index: HashMap<String, Vec<String>> = driver::load(&path).unwrap();

        // "rust" doit pointer vers "1"
        assert!(index.get("rust").unwrap().contains(&"1".to_string()));
        // "dev" doit pointer vers "1"
        assert!(index.get("dev").unwrap().contains(&"1".to_string()));
    }
}
