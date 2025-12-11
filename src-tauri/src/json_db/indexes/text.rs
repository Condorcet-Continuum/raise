use anyhow::Result;
use serde_json::Value;
use std::collections::{HashMap, HashSet};

// CORRECTION : On importe uniquement le module driver, le trait IndexMap n'est pas utilisé directement
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

/// Met à jour l'index Textuel (Index Inversé).
///
/// Structure : Token -> [DocId1, DocId2, ...]
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

    // On charge l'index existant (HashMap<Token, Vec<DocId>>)
    let mut index: HashMap<String, Vec<String>> = driver::load(&path)?;
    let mut changed = false;

    // 1. Suppression des anciens tokens
    if let Some(doc) = old_doc {
        if let Some(val) = doc.pointer(&def.field_path).and_then(|v| v.as_str()) {
            let tokens = tokenize(val);
            for token in tokens {
                if let Some(ids) = index.get_mut(&token) {
                    if let Some(pos) = ids.iter().position(|x| x == doc_id) {
                        ids.swap_remove(pos); // Suppression rapide O(1)
                        changed = true;
                    }
                }
                // Nettoyage si vide
                if index.get(&token).map(|ids| ids.is_empty()).unwrap_or(false) {
                    index.remove(&token);
                }
            }
        }
    }

    // 2. Ajout des nouveaux tokens
    if let Some(doc) = new_doc {
        if let Some(val) = doc.pointer(&def.field_path).and_then(|v| v.as_str()) {
            let tokens = tokenize(val);
            for token in tokens {
                let ids = index.entry(token).or_default();
                if !ids.contains(&doc_id.to_string()) {
                    ids.push(doc_id.to_string());
                    changed = true;
                }
            }
        }
    }

    // 3. Sauvegarde si modification
    if changed {
        driver::save(&path, &index)?;
    }

    Ok(())
}
