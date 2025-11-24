use anyhow::Result;
use serde_json::Value;
use std::collections::BTreeMap;

use super::{driver, paths, IndexDefinition};
use crate::json_db::storage::JsonDbConfig;

/// Met à jour l'index B-Tree (wrapper vers driver générique)
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
    // On spécifie le type BTreeMap pour garantir l'ordre des clés
    driver::update::<BTreeMap<String, Vec<String>>>(&path, def, doc_id, old_doc, new_doc)
}
