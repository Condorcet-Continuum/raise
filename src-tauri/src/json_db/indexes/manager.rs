use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fs, path::PathBuf};

use crate::json_db::{
    collections::collection,
    storage::{self, JsonDbConfig},
};

// CORRECTION : Ajout de `text` dans les imports
use super::{btree, hash, paths, text, IndexDefinition, IndexType};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CollectionConfig {
    pub schema_rel: String,
    #[serde(default)]
    pub indexes: Vec<IndexDefinition>,
}

fn collection_config_path(cfg: &JsonDbConfig, space: &str, db: &str, collection: &str) -> PathBuf {
    collection::collection_root(cfg, space, db, collection).join("_config.json")
}

pub fn get_collection_index_definitions(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
) -> Result<Vec<IndexDefinition>> {
    let path = collection_config_path(cfg, space, db, collection);

    if !path.exists() {
        return Ok(vec![IndexDefinition {
            name: "id".to_string(),
            field_path: "/id".to_string(),
            index_type: IndexType::Hash,
            unique: true,
        }]);
    }

    let content = fs::read_to_string(&path)
        .with_context(|| format!("Lecture config collection {}", path.display()))?;

    let config: CollectionConfig = serde_json::from_str(&content)
        .with_context(|| format!("Désérialisation config collection {}", path.display()))?;

    let has_id_index = config.indexes.iter().any(|def| def.name == "id");
    let mut definitions = config.indexes;

    if !has_id_index {
        definitions.push(IndexDefinition {
            name: "id".to_string(),
            field_path: "/id".to_string(),
            index_type: IndexType::Hash,
            unique: true,
        });
    }

    Ok(definitions)
}

pub fn create_collection_indexes(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    schema_rel: &str,
) -> Result<()> {
    let indexes_root = paths::indexes_root(cfg, space, db, collection);
    fs::create_dir_all(&indexes_root)
        .with_context(|| format!("Création répertoire index {}", indexes_root.display()))?;

    let config = CollectionConfig {
        schema_rel: schema_rel.to_string(),
        indexes: vec![IndexDefinition {
            name: "id".to_string(),
            field_path: "/id".to_string(),
            index_type: IndexType::Hash,
            unique: true,
        }],
    };

    let config_path = collection_config_path(cfg, space, db, collection);
    storage::file_storage::atomic_write_json(&config_path, &serde_json::to_value(config)?)?;

    Ok(())
}

pub fn update_indexes(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    collection: &str,
    doc_id: &str,
    old_doc: Option<&Value>,
    new_doc: Option<&Value>,
) -> Result<()> {
    let definitions = get_collection_index_definitions(cfg, space, db, collection)?;

    for def in definitions {
        match def.index_type {
            IndexType::BTree => btree::update_btree_index(
                cfg, space, db, collection, &def, doc_id, old_doc, new_doc,
            )
            .with_context(|| format!("Échec mise à jour BTree index: {}", def.name))?,

            IndexType::Hash => {
                hash::update_hash_index(cfg, space, db, collection, &def, doc_id, old_doc, new_doc)
                    .with_context(|| format!("Échec mise à jour Hash index: {}", def.name))?
            }

            // CORRECTION : Prise en charge du type Text
            IndexType::Text => {
                text::update_text_index(cfg, space, db, collection, &def, doc_id, old_doc, new_doc)
                    .with_context(|| format!("Échec mise à jour Text index: {}", def.name))?
            }
        }
    }

    Ok(())
}
