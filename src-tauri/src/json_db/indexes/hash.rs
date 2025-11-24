use anyhow::Result;
use serde_json::Value;
use std::collections::HashMap;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

/// Met à jour l'index de hachage (wrapper vers driver générique)
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
    // On spécifie le type HashMap pour utiliser l'implémentation IndexMap correspondante
    driver::update::<HashMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc)
}
