// FICHIER : src-tauri/src/json_db/indexes/text.rs

use anyhow::Result;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

/// Découpe un texte en tokens normalisés (minuscules, alphanumériques)
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

    // On charge manuellement car la logique de mise à jour est spécifique (Multi-clés par document)
    let mut index: HashMap<String, Vec<String>> = driver::load(&path)?;
    let mut changed = false;

    // Suppression des anciens tokens
    if let Some(doc) = old_doc {
        if let Some(val) = doc.pointer(&def.field_path).and_then(|v| v.as_str()) {
            for token in tokenize(val) {
                if let Some(ids) = index.get_mut(&token) {
                    if let Some(pos) = ids.iter().position(|x| x == doc_id) {
                        ids.swap_remove(pos);
                        changed = true;
                    }
                }
                // Nettoyage des clés vides
                if index.get(&token).is_some_and(|ids| ids.is_empty()) {
                    index.remove(&token);
                }
            }
        }
    }

    // Ajout des nouveaux tokens
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

/// Recherche simple de mot-clé (Token exact).
/// Note : Pour une recherche "phrase entière", il faudrait une intersection des résultats de chaque token.
pub fn search_text_index(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    def: &IndexDefinition,
    query: &str,
) -> Result<Vec<String>> {
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);

    // Normalisation de la requête pour matcher les tokens stockés
    let token = query.to_lowercase();

    // Utilisation du driver générique (Hashmap est la structure sous-jacente)
    driver::search::<HashMap<String, Vec<String>>>(&path, &token)
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
    fn test_text_lifecycle() {
        let (dir, cfg) = setup_env();
        let idx_dir = dir.path().join("s/d/collections/c/_indexes");
        std::fs::create_dir_all(&idx_dir).unwrap();

        let def = IndexDefinition {
            name: "bio".into(),
            field_path: "/bio".into(),
            index_type: IndexType::Text,
            unique: false,
        };

        // 1. Insertion "Rust is great" -> Tokens: [rust, is, great]
        let doc = json!({ "bio": "Rust is great" });
        update_text_index(&cfg, "s", "d", "c", &def, "u1", None, Some(&doc)).unwrap();

        // 2. Recherche "RUST" (Doit marcher grâce à la normalisation)
        let results = search_text_index(&cfg, "s", "d", "c", &def, "RUST").unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], "u1");

        // 3. Recherche mot partiel (Ne marche pas avec ce tokenizer simple, "gre" != "great")
        let partial = search_text_index(&cfg, "s", "d", "c", &def, "gre").unwrap();
        assert!(partial.is_empty());

        // 4. Suppression
        update_text_index(&cfg, "s", "d", "c", &def, "u1", Some(&doc), None).unwrap();
        let deleted = search_text_index(&cfg, "s", "d", "c", &def, "rust").unwrap();
        assert!(deleted.is_empty());
    }
}
