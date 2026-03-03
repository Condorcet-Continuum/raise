// FICHIER : src-tauri/src/json_db/indexes/text.rs

use super::{driver, paths, IndexDefinition};
// ✅ AJOUT : Import du StorageEngine
use crate::json_db::storage::StorageEngine;

use crate::utils::data::{HashMap, HashSet};
use crate::utils::prelude::*;

fn tokenize(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[allow(clippy::too_many_arguments)]
pub async fn update_text_index(
    storage: &StorageEngine, // ✅ MODIFIÉ
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> RaiseResult<()> {
    let path = paths::index_path(
        &storage.config,
        space,
        db,
        collection,
        &def.name,
        def.index_type,
    );

    let mut index: HashMap<String, Vec<String>> = driver::load(&path).await?;
    let mut changed = false;

    if let Some(doc) = old_doc {
        if let Some(val) = doc.pointer(&def.field_path).and_then(|v| v.as_str()) {
            for token in tokenize(val) {
                if let Some(ids) = index.get_mut(&token) {
                    if let Some(pos) = ids.iter().position(|x| x == doc_id) {
                        ids.swap_remove(pos);
                        changed = true;
                    }
                }
                if index.get(&token).is_some_and(|ids| ids.is_empty()) {
                    index.remove(&token);
                }
            }
        }
    }

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
        driver::save(&path, &index).await?;
    }

    Ok(())
}

pub async fn search_text_index(
    storage: &StorageEngine, // ✅ MODIFIÉ
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    query: &str,
) -> RaiseResult<Vec<String>> {
    let path = paths::index_path(
        &storage.config,
        space,
        db,
        collection,
        &def.name,
        def.index_type,
    );

    let token = query.to_lowercase();
    driver::search::<HashMap<String, Vec<String>>>(&path, &token).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::indexes::IndexType;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::{
        io::{self, tempdir},
        json::json,
    };

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let cfg = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, cfg)
    }

    #[tokio::test]
    async fn test_text_lifecycle() {
        let (dir, cfg) = setup_env();
        // ✅ AJOUT : Création du StorageEngine pour les tests
        let storage = StorageEngine::new(cfg);

        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        io::ensure_dir(&idx_dir).await.unwrap();

        let def = IndexDefinition {
            name: "bio".into(),
            field_path: "/bio".into(),
            index_type: IndexType::Text,
            unique: false,
        };

        let doc = json!({ "bio": "Rust is great" });
        update_text_index(&storage, "s", "d", "c", &def, "u1", None, Some(&doc))
            .await
            .unwrap();

        let results = search_text_index(&storage, "s", "d", "c", &def, "RUST")
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "u1");

        let partial = search_text_index(&storage, "s", "d", "c", &def, "gre")
            .await
            .unwrap();
        assert!(partial.is_empty());

        update_text_index(&storage, "s", "d", "c", &def, "u1", Some(&doc), None)
            .await
            .unwrap();
        let deleted = search_text_index(&storage, "s", "d", "c", &def, "rust")
            .await
            .unwrap();
        assert!(deleted.is_empty());
    }
}
