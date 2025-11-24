use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

/// Met à jour l'index Textuel.
///
/// Note: Pour l'instant, cela utilise le comportement "Hash" standard (correspondance exacte).
/// La tokenisation sera ajoutée dans une prochaine étape d'optimisation via une
/// implémentation spécifique du trait `IndexMap` ou une surcharge de `driver::update`.
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
    // On récupère le chemin correct pour un index de type Text
    let path = paths::index_path(cfg, space, db, collection, &def.name, def.index_type);

    // On utilise HashMap pour le stockage en mémoire (rapide pour les lookups exacts)
    driver::update::<HashMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc)
}
