// FICHIER : src-tauri/src/json_db/collections/manager.rs

use crate::json_db::indexes::IndexManager;
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
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
        // 1. Init DB structure
        file_storage::create_db(&self.storage.config, &self.space, &self.db)
            .context("Impossible d'initialiser la structure DB parente")?;

        // 2. Résolution du schéma
        let final_schema_uri = if let Some(uri) = schema_uri {
            uri
        } else {
            self.resolve_schema_from_index(name)?
        };

        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, name);

        // 3. Création Dossier
        if !col_path.exists() {
            fs::create_dir_all(&col_path)
                .with_context(|| format!("Échec création dossier collection : {:?}", col_path))?;
        }

        // 4. Écriture _meta.json (Initialise la liste des index vide)
        let meta = json!({
            "schema": final_schema_uri,
            "indexes": []
        });
        let meta_path = col_path.join("_meta.json");

        fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)
            .with_context(|| format!("Échec écriture métadonnées : {:?}", meta_path))?;

        // 5. Mise à jour Index Système
        self.update_system_index_collection(name, &final_schema_uri)
            .context("Échec mise à jour de l'index _system.json")?;

        Ok(())
    }

    fn resolve_schema_from_index(&self, col_name: &str) -> Result<String> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        if !sys_path.exists() {
            return Err(anyhow!("Index _system.json introuvable"));
        }

        let content = fs::read_to_string(&sys_path)?;
        let sys_json: Value = serde_json::from_str(&content)?;
        let ptr = format!("/collections/{}/schema", col_name);

        let raw_path = sys_json
            .pointer(&ptr)
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Collection '{}' inconnue dans _system.json", col_name))?;

        let relative_path = if let Some(idx) = raw_path.find("/schemas/v1/") {
            &raw_path[idx + "/schemas/v1/".len()..]
        } else {
            raw_path
        };

        let schema_path = self
            .storage
            .config
            .db_schemas_root(&self.space, &self.db)
            .join("v1")
            .join(relative_path);
        if !schema_path.exists() {
            return Err(anyhow!("Schéma introuvable : {:?}", schema_path));
        }

        Ok(format!(
            "db://{}/{}/schemas/v1/{}",
            self.space, self.db, relative_path
        ))
    }

    fn update_system_index_collection(&self, col_name: &str, schema_uri: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        let mut system_doc = if sys_path.exists() {
            serde_json::from_str(&fs::read_to_string(&sys_path)?)?
        } else {
            json!({ "collections": {} })
        };

        if let Some(cols) = system_doc["collections"].as_object_mut() {
            let existing_items = cols
                .get(col_name)
                .and_then(|c| c.get("items"))
                .cloned()
                .unwrap_or(json!([]));
            cols.insert(
                col_name.to_string(),
                json!({ "schema": schema_uri, "items": existing_items }),
            );
        } else {
            system_doc["collections"] = json!({ col_name: { "schema": schema_uri, "items": [] } });
        }
        fs::write(&sys_path, serde_json::to_string_pretty(&system_doc)?)?;
        Ok(())
    }

    fn add_item_to_index(&self, col_name: &str, id: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        let mut system_doc = if sys_path.exists() {
            serde_json::from_str(&fs::read_to_string(&sys_path)?)?
        } else {
            json!({ "collections": {} })
        };

        let filename = format!("{}.json", id);
        if let Some(cols) = system_doc["collections"].as_object_mut() {
            if let Some(col_entry) = cols.get_mut(col_name) {
                if col_entry.get("items").is_none() {
                    col_entry["items"] = json!([]);
                }
                if let Some(items) = col_entry["items"].as_array_mut() {
                    if !items
                        .iter()
                        .any(|i| i.get("file").and_then(|f| f.as_str()) == Some(&filename))
                    {
                        items.push(json!({ "file": filename }));
                    }
                }
            }
        }
        fs::write(&sys_path, serde_json::to_string_pretty(&system_doc)?)?;
        Ok(())
    }

    pub fn list_collections(&self) -> Result<Vec<String>> {
        let root = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("collections");
        let mut cols = Vec::new();
        if root.exists() {
            for entry in fs::read_dir(root)? {
                let entry = entry?;
                if entry.path().is_dir() {
                    if let Some(name) = entry.file_name().to_str() {
                        if !name.starts_with('_') {
                            cols.push(name.to_string());
                        }
                    }
                }
            }
        }
        Ok(cols)
    }

    pub fn insert_raw(&self, collection: &str, doc: &Value) -> Result<()> {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("ID manquant"))?;

        // 1. Écriture disque
        self.storage
            .write_document(&self.space, &self.db, collection, id, doc)?;

        // 2. Index Système
        self.add_item_to_index(collection, id)?;

        // 3. INDEXATION SECONDAIRE
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        if let Err(e) = idx_mgr.index_document(collection, doc) {
            #[cfg(debug_assertions)]
            eprintln!("Indexation error for {}: {}", id, e);
        }

        Ok(())
    }

    pub fn insert_with_schema(&self, collection: &str, mut doc: Value) -> Result<Value> {
        self.prepare_document(collection, &mut doc)?;
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
        let old_doc = self.get_document(collection, id)?;
        if old_doc.is_none() {
            return Err(anyhow!("Document introuvable"));
        }

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("id".to_string(), Value::String(id.to_string()));
        }

        self.prepare_document(collection, &mut doc)?;
        self.storage
            .write_document(&self.space, &self.db, collection, id, &doc)?;

        // INDEXATION (Clean old -> Add new)
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        if let Some(old) = old_doc {
            let _ = idx_mgr.remove_document(collection, &old);
        }
        let _ = idx_mgr.index_document(collection, &doc);

        Ok(doc)
    }

    pub fn delete_document(&self, collection: &str, id: &str) -> Result<bool> {
        let old_doc = self.get_document(collection, id)?;
        self.storage
            .delete_document(&self.space, &self.db, collection, id)?;

        // INDEXATION (Clean)
        if let Some(doc) = old_doc {
            let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
            let _ = idx_mgr.remove_document(collection, &doc);
        }

        Ok(true)
    }

    pub fn list_all(&self, collection: &str) -> Result<Vec<Value>> {
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection);
        let mut docs = Vec::new();
        if col_path.exists() {
            for entry in fs::read_dir(&col_path)? {
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
            let content = fs::read_to_string(&meta_path)?;
            let meta: Value = serde_json::from_str(&content)?;
            meta.get("schema")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        } else {
            None
        };

        // 1. Validation JSON Schema (Structurelle)
        if let Some(uri) = schema_uri {
            if !uri.is_empty() {
                if let Some(obj) = doc.as_object_mut() {
                    if !obj.contains_key("$schema") {
                        obj.insert("$schema".to_string(), Value::String(uri.clone()));
                    }
                }
                let reg = SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db)?;
                let validator = SchemaValidator::compile_with_registry(&uri, &reg)?;
                validator.compute_then_validate(doc)?;
            }
        }

        // 2. Validation Sémantique (JSON-LD)
        // CORRECTION : Appel effectif de la fonction pour enrichir le document
        self.apply_semantic_logic(doc)
            .context("Validation sémantique")?;

        Ok(())
    }

    /// Nouvelle fonction : Enrichit et valide sémantiquement le document
    fn apply_semantic_logic(&self, doc: &mut Value) -> Result<()> {
        // A. Injection automatique du contexte Arcadia si absent
        if let Some(obj) = doc.as_object_mut() {
            if !obj.contains_key("@context") {
                obj.insert(
                    "@context".to_string(),
                    json!({
                        "oa": "https://genaptitude.io/ontology/arcadia/oa#",
                        "sa": "https://genaptitude.io/ontology/arcadia/sa#",
                        "la": "https://genaptitude.io/ontology/arcadia/la#",
                        "pa": "https://genaptitude.io/ontology/arcadia/pa#"
                    }),
                );
            }
        }

        // B. Validation des Types (@type) contre l'ontologie
        let processor = JsonLdProcessor::new();

        if let Some(type_uri) = processor.get_type(doc) {
            let registry = VocabularyRegistry::new();
            let expanded_type = processor.context_manager().expand_term(&type_uri);

            if !registry.has_class(&expanded_type) {
                #[cfg(debug_assertions)]
                println!(
                    "⚠️  [Semantic Warning] Type inconnu de l'ontologie : {}",
                    expanded_type
                );
            }
        }

        Ok(())
    }
}
