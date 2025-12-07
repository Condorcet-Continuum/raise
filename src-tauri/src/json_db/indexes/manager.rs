// FICHIER : src-tauri/src/json_db/indexes/manager.rs

use super::{btree, hash, text, IndexDefinition, IndexType};
use crate::json_db::storage::StorageEngine;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;

// Structure interne pour lire _meta.json
#[derive(Debug, Serialize, Deserialize)]
struct CollectionMeta {
    #[serde(default)]
    pub schema: Option<String>,
    #[serde(default)]
    pub indexes: Vec<IndexDefinition>,
}

pub struct IndexManager<'a> {
    storage: &'a StorageEngine,
    space: String,
    db: String,
}

impl<'a> IndexManager<'a> {
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            storage,
            space: space.to_string(),
            db: db.to_string(),
        }
    }

    /// Charge les définitions d'index depuis le fichier _meta.json de la collection
    fn load_indexes(&self, collection: &str) -> Result<Vec<IndexDefinition>> {
        let meta_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");

        if !meta_path.exists() {
            return Ok(Vec::new());
        }

        let content = fs::read_to_string(&meta_path)
            .with_context(|| format!("Lecture meta impossible : {:?}", meta_path))?;

        let meta: CollectionMeta =
            serde_json::from_str(&content).unwrap_or_else(|_| CollectionMeta {
                schema: None,
                indexes: vec![],
            });

        Ok(meta.indexes)
    }

    /// Indexe un nouveau document (ou une mise à jour)
    pub fn index_document(&mut self, collection: &str, new_doc: &Value) -> Result<()> {
        let doc_id = new_doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Document sans ID, indexation impossible"))?;

        let indexes = self.load_indexes(collection)?;

        for def in indexes {
            // Pour l'instant, on traite l'ajout (new_doc).
            // La suppression de l'ancienne clé est gérée par remove_document avant.
            self.dispatch_update(collection, &def, doc_id, None, Some(new_doc))?;
        }

        Ok(())
    }

    /// Retire un document des index
    pub fn remove_document(&mut self, collection: &str, old_doc: &Value) -> Result<()> {
        // Si le document est null (ex: suppression par ID sans lecture préalable), on ne peut pas désindexer.
        // C'est la responsabilité du CollectionsManager de fournir l'ancien doc.
        if old_doc.is_null() {
            return Ok(());
        }

        let doc_id = old_doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
        if doc_id.is_empty() {
            return Ok(());
        }

        let indexes = self.load_indexes(collection)?;

        for def in indexes {
            self.dispatch_update(collection, &def, doc_id, Some(old_doc), None)?;
        }

        Ok(())
    }

    /// Dispatch vers l'implémentation spécifique (Hash, BTree, Text)
    fn dispatch_update(
        &self,
        collection: &str,
        def: &IndexDefinition,
        doc_id: &str,
        old: Option<&Value>,
        new: Option<&Value>,
    ) -> Result<()> {
        match def.index_type {
            IndexType::Hash => hash::update_hash_index(
                &self.storage.config,
                &self.space,
                &self.db,
                collection,
                def,
                doc_id,
                old,
                new,
            ),
            IndexType::BTree => btree::update_btree_index(
                &self.storage.config,
                &self.space,
                &self.db,
                collection,
                def,
                doc_id,
                old,
                new,
            ),
            IndexType::Text => text::update_text_index(
                &self.storage.config,
                &self.space,
                &self.db,
                collection,
                def,
                doc_id,
                old,
                new,
            ),
        }
        .with_context(|| format!("Erreur mise à jour index '{}'", def.name))
    }
}

/// Helper pour ajouter un index à une collection (mise à jour de _meta.json)
pub fn add_index_definition(
    storage: &StorageEngine,
    space: &str,
    db: &str,
    collection: &str,
    def: IndexDefinition,
) -> Result<()> {
    let meta_path = storage
        .config
        .db_collection_path(space, db, collection)
        .join("_meta.json");

    let mut meta: CollectionMeta = if meta_path.exists() {
        serde_json::from_str(&fs::read_to_string(&meta_path)?)?
    } else {
        CollectionMeta {
            schema: None,
            indexes: vec![],
        }
    };

    // Éviter les doublons de nom
    if meta.indexes.iter().any(|i| i.name == def.name) {
        return Ok(());
    }

    meta.indexes.push(def);

    // Sauvegarde atomique simulée
    fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

    Ok(())
}
