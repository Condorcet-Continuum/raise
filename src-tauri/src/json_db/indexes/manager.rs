// FICHIER : src-tauri/src/json_db/indexes/manager.rs

use super::{btree, hash, text, IndexDefinition, IndexType};
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

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

    /// Crée un nouvel index (Mise à jour de _meta.json + Backfill)
    pub fn create_index(&mut self, collection: &str, field: &str, kind_str: &str) -> Result<()> {
        // 1. Validation du type
        let kind = match kind_str.to_lowercase().as_str() {
            "hash" => IndexType::Hash,
            "btree" => IndexType::BTree,
            "text" => IndexType::Text,
            _ => return Err(anyhow!("Type d'index inconnu: {}", kind_str)),
        };

        // 2. Construction de la définition
        let field_path = if field.starts_with('/') {
            field.to_string()
        } else {
            format!("/{}", field)
        };

        let def = IndexDefinition {
            name: field.to_string(),
            field_path,
            index_type: kind,
            unique: false,
        };

        // 3. Mise à jour de _meta.json via la fonction statique (DRY)
        add_index_definition(self.storage, &self.space, &self.db, collection, def.clone())?;

        // 4. Backfill (Reconstruction)
        self.rebuild_index(collection, &def)?;

        Ok(())
    }

    /// Supprime un index existant
    pub fn drop_index(&mut self, collection: &str, field: &str) -> Result<()> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            return Err(anyhow!("Collection introuvable ou sans métadonnées"));
        }

        let mut meta = self.load_meta(&meta_path)?;

        if let Some(pos) = meta.indexes.iter().position(|i| i.name == field) {
            let removed = meta.indexes.remove(pos);

            // Sauvegarde Meta
            fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

            // Suppression Physique
            let index_filename = match removed.index_type {
                IndexType::Hash => format!("{}.hash.idx", removed.name),
                IndexType::BTree => format!("{}.btree.idx", removed.name),
                IndexType::Text => format!("{}.text.idx", removed.name),
            };

            let index_path = self
                .storage
                .config
                .db_collection_path(&self.space, &self.db, collection)
                .join("_indexes")
                .join(index_filename);

            if index_path.exists() {
                fs::remove_file(index_path)?;
            }
        } else {
            return Err(anyhow!("Index introuvable pour le champ '{}'", field));
        }

        Ok(())
    }

    /// Reconstruit un index en parcourant tous les documents
    fn rebuild_index(&self, collection: &str, def: &IndexDefinition) -> Result<()> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);
        let indexes_dir = col_path.join("_indexes");
        if !indexes_dir.exists() {
            fs::create_dir_all(&indexes_dir)?;
        }

        println!(
            "🔄 Reconstruction de l'index {} sur {}...",
            def.name, def.field_path
        );

        for entry in fs::read_dir(&col_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json") {
                let filename = path.file_name().unwrap().to_str().unwrap();
                if filename.starts_with('_') {
                    continue;
                }

                let content = fs::read_to_string(&path)?;
                if let Ok(doc) = serde_json::from_str::<Value>(&content) {
                    let doc_id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if !doc_id.is_empty() {
                        self.dispatch_update(collection, def, doc_id, None, Some(&doc))?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Indexe un document (ajout/mise à jour)
    pub fn index_document(&mut self, collection: &str, new_doc: &Value) -> Result<()> {
        let doc_id = new_doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Document sans ID"))?;

        let indexes = self.load_indexes(collection)?;

        // Création du dossier _indexes si nécessaire
        if !indexes.is_empty() {
            let indexes_dir = self
                .storage
                .config
                .db_collection_path(&self.space, &self.db, collection)
                .join("_indexes");
            if !indexes_dir.exists() {
                fs::create_dir_all(indexes_dir)?;
            }
        }

        for def in indexes {
            self.dispatch_update(collection, &def, doc_id, None, Some(new_doc))?;
        }
        Ok(())
    }

    /// Retire un document des index
    pub fn remove_document(&mut self, collection: &str, old_doc: &Value) -> Result<()> {
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

    // --- Helpers Privés ---

    fn load_indexes(&self, collection: &str) -> Result<Vec<IndexDefinition>> {
        let meta_path = self.get_meta_path(collection);
        if !meta_path.exists() {
            return Ok(Vec::new());
        }
        let meta = self.load_meta(&meta_path)?;
        Ok(meta.indexes)
    }

    fn get_meta_path(&self, collection: &str) -> PathBuf {
        self.storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json")
    }

    fn load_meta(&self, path: &PathBuf) -> Result<CollectionMeta> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Lecture meta impossible : {:?}", path))?;
        serde_json::from_str(&content).map_err(|e| anyhow!("Erreur parsing _meta.json: {}", e))
    }

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
} // <--- C'est l'accolade qui manquait probablement !

// --- FONCTION STATIQUE (HORS IMPL) ---

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

    fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;

    Ok(())
}
