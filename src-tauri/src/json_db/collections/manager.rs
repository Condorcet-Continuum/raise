// FICHIER : src-tauri/src/json_db/collections/manager.rs

use crate::json_db::indexes::IndexManager;
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::{file_storage, StorageEngine};
use anyhow::{anyhow, Context, Result};
use serde_json::{json, Value};
use std::fs;
use uuid::Uuid;

#[derive(Debug)]
pub struct CollectionsManager<'a> {
    pub storage: &'a StorageEngine,
    pub space: String,
    pub db: String,
}

impl<'a> CollectionsManager<'a> {
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            storage,
            space: space.to_string(),
            db: db.to_string(),
        }
    }

    pub fn create_collection(&self, name: &str, schema_uri: Option<String>) -> Result<()> {
        // 1. Init DB parente
        file_storage::create_db(&self.storage.config, &self.space, &self.db)
            .context("Impossible d'initialiser la structure DB parente")?;

        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, name);

        // 2. Création Dossier
        if !col_path.exists() {
            fs::create_dir_all(&col_path)
                .with_context(|| format!("Échec création dossier collection : {:?}", col_path))?;
        }

        // 3. Écriture _meta.json
        let uri_str = schema_uri.clone().unwrap_or_default();
        let meta = json!({ "schema": uri_str });
        let meta_path = col_path.join("_meta.json");

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("Échec écriture métadonnées : {:?}", meta_path))?;

        // 4. Mise à jour Index Système
        self.update_system_index_collection(name, &uri_str)
            .context("Échec mise à jour de l'index _system.json")?;

        Ok(())
    }

    fn update_system_index_collection(&self, col_name: &str, schema_uri: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        // Chargement robuste
        let mut system_doc = if sys_path.exists() {
            let content = fs::read_to_string(&sys_path)
                .with_context(|| format!("Lecture impossible de _system.json à {:?}", sys_path))?;
            serde_json::from_str(&content).with_context(|| "Parsing JSON de _system.json échoué")?
        } else {
            // Si pas d'index, on part d'un vide
            eprintln!(
                "⚠️  Attention: _system.json introuvable à {:?}, création d'un nouvel index.",
                sys_path
            );
            json!({ "collections": {} })
        };

        // Modification
        if let Some(cols) = system_doc["collections"].as_object_mut() {
            let existing_items = cols
                .get(col_name)
                .and_then(|c| c.get("items"))
                .cloned()
                .unwrap_or(json!([]));

            cols.insert(
                col_name.to_string(),
                json!({
                    "schema": schema_uri,
                    "items": existing_items
                }),
            );
        } else {
            // Réparation structurelle si nécessaire
            system_doc["collections"] = json!({
                col_name: {
                    "schema": schema_uri,
                    "items": []
                }
            });
        }

        fs::write(&sys_path, serde_json::to_string_pretty(&system_doc)?)
            .with_context(|| format!("Écriture impossible de _system.json à {:?}", sys_path))?;

        Ok(())
    }

    fn add_item_to_index(&self, col_name: &str, id: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        let mut system_doc = self.load_system_index(&sys_path);
        let filename = format!("{}.json", id);

        if let Some(cols) = system_doc["collections"].as_object_mut() {
            if let Some(col_entry) = cols.get_mut(col_name) {
                if col_entry.get("items").is_none() {
                    col_entry["items"] = json!([]);
                }
                if let Some(items) = col_entry["items"].as_array_mut() {
                    let exists = items
                        .iter()
                        .any(|item| item.get("file").and_then(|f| f.as_str()) == Some(&filename));
                    if !exists {
                        items.push(json!({ "file": filename }));
                    }
                }
            }
        }
        fs::write(&sys_path, serde_json::to_string_pretty(&system_doc)?)
            .with_context(|| format!("Mise à jour index item échouée : {:?}", sys_path))?;
        Ok(())
    }

    fn load_system_index(&self, path: &std::path::Path) -> Value {
        if path.exists() {
            let content = fs::read_to_string(path).unwrap_or_default();
            serde_json::from_str(&content).unwrap_or(json!({ "collections": {} }))
        } else {
            json!({ "collections": {} })
        }
    }

    pub fn list_collections(&self) -> Result<Vec<String>> {
        let collections_root = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("collections");

        let mut collections = Vec::new();
        if collections_root.exists() {
            for entry in fs::read_dir(&collections_root)
                .with_context(|| format!("Lecture dossier collections : {:?}", collections_root))?
            {
                let entry = entry?;
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('_') {
                            collections.push(name.to_string());
                        }
                    }
                }
            }
        }
        Ok(collections)
    }

    pub fn list_collection_names(&self) -> Result<Vec<String>> {
        self.list_collections()
    }

    pub fn insert_raw(&self, collection: &str, doc: &Value) -> Result<()> {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("insert_raw: Document sans ID"))?;

        self.storage
            .write_document(&self.space, &self.db, collection, id, doc)
            .context("Erreur écriture physique document")?;

        self.add_item_to_index(collection, id)
            .context("Erreur mise à jour index système")?;

        let mut _idx = IndexManager::new(self.storage, &self.space, &self.db);
        Ok(())
    }

    pub fn insert_with_schema(&self, collection: &str, mut doc: Value) -> Result<Value> {
        self.prepare_document(collection, &mut doc)
            .context(format!("Validation schéma échouée pour '{}'", collection))?;

        if doc.get("id").is_none() {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("id".to_string(), Value::String(Uuid::new_v4().to_string()));
            }
        }

        self.insert_raw(collection, &doc)?;
        Ok(doc)
    }

    pub fn get_document(&self, collection: &str, id: &str) -> Result<Option<Value>> {
        self.storage
            .read_document(&self.space, &self.db, collection, id)
    }

    pub fn get(&self, collection: &str, id: &str) -> Result<Option<Value>> {
        self.get_document(collection, id)
    }

    pub fn update_document(&self, collection: &str, id: &str, mut doc: Value) -> Result<Value> {
        if self.get_document(collection, id)?.is_none() {
            return Err(anyhow!("Document introuvable : {}/{}", collection, id));
        }
        if let Some(obj) = doc.as_object_mut() {
            obj.insert("id".to_string(), Value::String(id.to_string()));
        }

        if let Err(e) = self.prepare_document(collection, &mut doc) {
            #[cfg(debug_assertions)]
            eprintln!("Schema warning update {}: {}", collection, e);
        }

        self.storage
            .write_document(&self.space, &self.db, collection, id, &doc)?;
        Ok(doc)
    }

    pub fn delete_document(&self, collection: &str, id: &str) -> Result<bool> {
        self.storage
            .delete_document(&self.space, &self.db, collection, id)?;
        Ok(true)
    }

    pub fn list_all(&self, collection: &str) -> Result<Vec<Value>> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);

        let mut docs = Vec::new();
        if col_path.exists() {
            for entry in fs::read_dir(&col_path)
                .with_context(|| format!("Lecture dossier collection : {:?}", col_path))?
            {
                let entry = entry?;
                let path = entry.path();
                if path.extension().map_or(false, |e| e == "json") {
                    if path.file_name().unwrap() == "_meta.json" {
                        continue;
                    }
                    let content = fs::read_to_string(&path)?;
                    if let Ok(doc) = serde_json::from_str::<Value>(&content) {
                        docs.push(doc);
                    }
                }
            }
        }
        Ok(docs)
    }

    fn prepare_document(&self, collection: &str, doc: &mut Value) -> Result<()> {
        let meta_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");

        let schema_uri = if meta_path.exists() {
            let content = fs::read_to_string(&meta_path)
                .with_context(|| format!("Lecture _meta.json impossible : {:?}", meta_path))?;
            let meta: Value = serde_json::from_str(&content)
                .with_context(|| format!("JSON _meta.json invalide : {:?}", meta_path))?;

            meta.get("schema")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        if let Some(uri) = schema_uri {
            if uri.is_empty() {
                return Ok(());
            }

            // --- CORRECTION ICI ---
            // On injecte l'URI du schéma dans le document AVANT le calcul
            // Cela permet de satisfaire la règle update: "if_missing" du type "instanceSchemaUri"
            if let Some(obj) = doc.as_object_mut() {
                // Si le champ $schema n'existe pas ou est vide, on le remplit
                if !obj.contains_key("$schema")
                    || obj
                        .get("$schema")
                        .and_then(|s| s.as_str())
                        .unwrap_or("")
                        .is_empty()
                {
                    obj.insert("$schema".to_string(), Value::String(uri.clone()));
                }
            }
            // -----------------------

            let reg = SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db)
                .context("Chargement du registre de schémas échoué")?;

            let validator = SchemaValidator::compile_with_registry(&uri, &reg)
                .with_context(|| format!("Schéma introuvable ou invalide : {}", uri))?;

            validator
                .compute_then_validate(doc)
                .context("Validation/Calcul du document échoué")?;
        }
        Ok(())
    }
}
