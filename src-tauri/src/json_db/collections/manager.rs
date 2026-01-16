// FICHIER : src-tauri/src/json_db/collections/manager.rs

use crate::json_db::indexes::IndexManager;
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::{file_storage, StorageEngine};
use crate::rules_engine::{EvalError, Evaluator, Rule, RuleStore};

use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use rayon::prelude::*; // Ajout pour le parallélisme
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use uuid::Uuid;

use super::collection;
use super::data_provider::CachedDataProvider;

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

    pub fn init_db(&self) -> Result<()> {
        file_storage::create_db(&self.storage.config, &self.space, &self.db)?;
        self.ensure_system_index()
    }

    // --- MÉTHODES DE LECTURE ---

    pub fn get_document(&self, collection: &str, id: &str) -> Result<Option<Value>> {
        match collection::read_document(&self.storage.config, &self.space, &self.db, collection, id)
        {
            Ok(doc) => Ok(Some(doc)),
            Err(_) => Ok(None),
        }
    }

    pub fn get(&self, collection: &str, id: &str) -> Result<Option<Value>> {
        self.get_document(collection, id)
    }

    /// Récupère plusieurs documents par leurs IDs en parallèle.
    /// Optimisation pour les requêtes indexées.
    /// Mode : STRICT INTEGRITY -> Panique si un fichier indexé est manquant.
    pub fn read_many(&self, collection: &str, ids: &[String]) -> Result<Vec<Value>> {
        // Utilisation de Rayon pour paralléliser les lectures I/O
        let results: Result<Vec<Value>> = ids
            .par_iter()
            .map(|id| {
                // Lecture unitaire
                let doc_opt = self.get_document(collection, id)
                    .with_context(|| format!("Erreur I/O lors de la lecture de l'ID {}", id))?;
                // Vérification d'intégrité stricte (Option B)
                match doc_opt {
                    Some(doc) => Ok(doc),
                    None => Err(anyhow!(
                        "DATABASE CORRUPTION: L'index pointe vers l'ID '{}' mais le fichier est introuvable dans '{}'", 
                        id, collection
                    )),
                }
            })
            .collect();

        results
    }

    pub fn list_all(&self, collection: &str) -> Result<Vec<Value>> {
        collection::list_documents(&self.storage.config, &self.space, &self.db, collection)
    }

    pub fn list_collections(&self) -> Result<Vec<String>> {
        collection::list_collection_names_fs(&self.storage.config, &self.space, &self.db)
    }

    // --- GESTION INDEX SYSTÈME ---

    pub fn ensure_system_index(&self) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        let mut system_doc = if sys_path.exists() {
            serde_json::from_str(&fs::read_to_string(&sys_path)?)?
        } else {
            json!({
                "space": self.space,
                "database": self.db,
                "version": 1,
                "collections": {}
            })
        };

        self.save_system_index(&mut system_doc)
    }

    fn save_system_index(&self, doc: &mut Value) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        let expected_uri = format!(
            "db://{}/{}/schemas/v1/db/index.schema.json",
            self.space, self.db
        );

        if let Some(obj) = doc.as_object_mut() {
            if !obj.contains_key("$schema") {
                obj.insert("$schema".to_string(), Value::String(expected_uri.clone()));
            }
            if !obj.contains_key("id") {
                obj.insert("id".to_string(), Value::String(Uuid::new_v4().to_string()));
            }
            let now = Utc::now().to_rfc3339();
            if !obj.contains_key("createdAt") {
                obj.insert("createdAt".to_string(), Value::String(now.clone()));
            }
            if !obj.contains_key("updatedAt") {
                obj.insert("updatedAt".to_string(), Value::String(now));
            }
        }

        let reg = SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db)?;

        let found_uri = if reg.get_by_uri(&expected_uri).is_some() {
            Some(expected_uri.clone())
        } else {
            reg.list_uris()
                .into_iter()
                .find(|u| u.ends_with("/db/index.schema.json"))
        };

        if let Some(uri) = found_uri {
            if let Ok(validator) = SchemaValidator::compile_with_registry(&uri, &reg) {
                if let Err(e) = validator.compute_then_validate(doc) {
                    return Err(anyhow!("Index système invalide: {}", e));
                }
            }
        }

        fs::write(&sys_path, serde_json::to_string_pretty(doc)?)?;
        Ok(())
    }

    // --- GESTION DES COLLECTIONS ---

    pub fn create_collection(&self, name: &str, schema_uri: Option<String>) -> Result<()> {
        if !self.storage.config.db_root(&self.space, &self.db).exists() {
            self.init_db()?;
        }

        let final_schema_uri = if let Some(uri) = schema_uri {
            uri
        } else {
            self.resolve_schema_from_index(name).unwrap_or_default()
        };

        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, name);
        if !col_path.exists() {
            fs::create_dir_all(&col_path)?;
        }

        let meta = json!({ "schema": final_schema_uri, "indexes": [] });
        let meta_path = col_path.join("_meta.json");
        if !meta_path.exists() {
            fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        }

        self.update_system_index_collection(name, &final_schema_uri)?;
        Ok(())
    }

    pub fn drop_collection(&self, name: &str) -> Result<()> {
        collection::drop_collection(&self.storage.config, &self.space, &self.db, name)?;
        self.remove_collection_from_system_index(name)?;
        Ok(())
    }

    // --- INDEXES SECONDAIRES ---

    pub fn create_index(&self, collection: &str, field: &str, kind: &str) -> Result<()> {
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        idx_mgr.create_index(collection, field, kind)
    }

    pub fn drop_index(&self, collection: &str, field: &str) -> Result<()> {
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        idx_mgr.drop_index(collection, field)
    }

    // --- HELPER INDEX SYSTÈME ---

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
            .ok_or_else(|| anyhow!("Collection '{}' inconnue", col_name))?;
        let relative_path = if let Some(idx) = raw_path.find("/schemas/v1/") {
            &raw_path[idx + "/schemas/v1/".len()..]
        } else {
            raw_path
        };
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
            json!({ "space": self.space, "database": self.db, "version": 1, "collections": {} })
        };

        if system_doc.get("collections").is_none() {
            system_doc["collections"] = json!({});
        }
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
        }
        self.save_system_index(&mut system_doc)?;
        Ok(())
    }

    fn remove_collection_from_system_index(&self, col_name: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        if !sys_path.exists() {
            return Ok(());
        }
        let content = fs::read_to_string(&sys_path)?;
        let mut system_doc: Value = serde_json::from_str(&content)?;
        let mut changed = false;
        if let Some(cols) = system_doc
            .get_mut("collections")
            .and_then(|c| c.as_object_mut())
        {
            if cols.remove(col_name).is_some() {
                changed = true;
            }
        }
        if changed {
            self.save_system_index(&mut system_doc)?;
        }
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
            json!({ "space": self.space, "database": self.db, "version": 1, "collections": {} })
        };

        if system_doc.get("collections").is_none() {
            system_doc["collections"] = json!({});
        }
        let filename = format!("{}.json", id);

        if let Some(cols) = system_doc["collections"].as_object_mut() {
            if !cols.contains_key(col_name) {
                let schema_guess = self
                    .resolve_schema_from_index(col_name)
                    .ok()
                    .unwrap_or_default();
                cols.insert(
                    col_name.to_string(),
                    json!({ "schema": schema_guess, "items": [] }),
                );
            }
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
        self.save_system_index(&mut system_doc)?;
        Ok(())
    }

    // --- ÉCRITURE ET MISE À JOUR ---

    pub fn insert_raw(&self, collection: &str, doc: &Value) -> Result<()> {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("ID manquant"))?;
        let meta_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");
        if !meta_path.exists() {
            let schema_hint = doc
                .get("$schema")
                .and_then(|s| s.as_str())
                .map(|s| s.to_string());
            self.create_collection(collection, schema_hint)?;
        }
        self.storage
            .write_document(&self.space, &self.db, collection, id, doc)?;
        self.add_item_to_index(collection, id)?;
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        if let Err(_e) = idx_mgr.index_document(collection, doc) {
            #[cfg(debug_assertions)]
            eprintln!("⚠️ Indexation secondaire échouée: {}", _e);
        }
        Ok(())
    }

    pub fn insert_with_schema(&self, collection: &str, mut doc: Value) -> Result<Value> {
        self.prepare_document(collection, &mut doc)?;
        self.insert_raw(collection, &doc)?;
        Ok(doc)
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
        if let Some(doc) = old_doc {
            let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
            let _ = idx_mgr.remove_document(collection, &doc);
        }
        Ok(true)
    }

    fn prepare_document(&self, collection: &str, doc: &mut Value) -> Result<()> {
        if let Some(obj) = doc.as_object_mut() {
            if !obj.contains_key("id") {
                obj.insert("id".to_string(), Value::String(Uuid::new_v4().to_string()));
            }
            let now = Utc::now().to_rfc3339();
            if !obj.contains_key("createdAt") {
                obj.insert("createdAt".to_string(), Value::String(now.clone()));
            }
            if !obj.contains_key("updatedAt") {
                obj.insert("updatedAt".to_string(), Value::String(now));
            }
        }

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

        if let Some(uri) = schema_uri {
            if !uri.is_empty() {
                if let Some(obj) = doc.as_object_mut() {
                    if !obj.contains_key("$schema") {
                        obj.insert("$schema".to_string(), Value::String(uri.clone()));
                    }
                }
                let reg = SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db)?;

                if let Err(e) = apply_business_rules(self, collection, doc, None, &reg, &uri) {
                    eprintln!("⚠️ Erreur règles métier (non bloquant): {}", e);
                }

                let validator = SchemaValidator::compile_with_registry(&uri, &reg)?;
                validator.compute_then_validate(doc)?;
            }
        }
        self.apply_semantic_logic(doc)
            .context("Validation sémantique")?;
        Ok(())
    }

    fn apply_semantic_logic(&self, doc: &mut Value) -> Result<()> {
        if let Some(obj) = doc.as_object_mut() {
            if !obj.contains_key("@context") {
                obj.insert(
                    "@context".to_string(),
                    json!({
                            "oa": "https://raise.io/ontology/arcadia/oa#",
                            "sa": "https://raise.io/ontology/arcadia/sa#",
                            "la": "https://raise.io/ontology/arcadia/la#",
                            "pa": "https://raise.io/ontology/arcadia/pa#",
                            "data": "https://raise.io/ontology/arcadia/data#"
                    }),
                );
            }
        }
        let processor = JsonLdProcessor::new();
        if let Some(type_uri) = processor.get_type(doc) {
            let registry = VocabularyRegistry::new();
            let expanded_type = processor.context_manager().expand_term(&type_uri);
            if !registry.has_class(&expanded_type) {
                #[cfg(debug_assertions)]
                println!("⚠️ [Semantic Warning] Type inconnu: {}", expanded_type);
            }
        }
        Ok(())
    }
}

// --- RULES ENGINE PIPELINE ---

#[allow(clippy::too_many_arguments)]
pub fn apply_business_rules(
    manager: &CollectionsManager,
    collection_name: &str,
    doc: &mut Value,
    old_doc: Option<&Value>,
    registry: &SchemaRegistry,
    schema_uri: &str,
) -> Result<()> {
    let mut store = RuleStore::new(manager);

    // ETAPE 1 : Charger toutes les règles existantes (cross-collection)
    if let Err(e) = store.sync_from_db() {
        eprintln!(
            "⚠️ Warning: Impossible de charger les règles système: {}",
            e
        );
    }

    // ETAPE 2 : Mettre à jour avec les règles du schéma courant
    if let Some(schema) = registry.get_by_uri(schema_uri) {
        if let Some(rules_array) = schema.get("x_rules").and_then(|v| v.as_array()) {
            for (index, rule_val) in rules_array.iter().enumerate() {
                match serde_json::from_value::<Rule>(rule_val.clone()) {
                    Ok(rule) => {
                        if let Err(e) = store.register_rule(collection_name, rule) {
                            eprintln!("⚠️ Erreur enregistrement règle (index {}): {}", index, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("⚠️ Règle invalide dans le schéma (index {}): {}", index, e);
                    }
                }
            }
        }
    }

    // ETAPE 3 : Création du DataProvider Optimisé avec Cache
    // Il vivra le temps de cette fonction (donc de la transaction)
    let provider = CachedDataProvider::new(&manager.storage.config, &manager.space, &manager.db);

    let mut current_changes = compute_diff(doc, old_doc);
    let mut passes = 0;
    const MAX_PASSES: usize = 10;

    while !current_changes.is_empty() && passes < MAX_PASSES {
        let rules = store.get_impacted_rules(collection_name, &current_changes);

        if rules.is_empty() {
            break;
        }

        let mut next_changes = HashSet::new();

        for rule in rules {
            match Evaluator::evaluate(&rule.expr, doc, &provider) {
                Ok(result) => {
                    // On convertit le Cow en Value (Owned) avant l'insertion
                    if set_value_by_path(doc, &rule.target, result.into_owned()) {
                        next_changes.insert(rule.target.clone());
                    }
                }
                Err(EvalError::VarNotFound(_)) => continue,
                Err(e) => return Err(anyhow!("Erreur calcul règle '{}': {}", rule.id, e)),
            }
        }

        current_changes = next_changes;
        passes += 1;
    }

    Ok(())
}

fn compute_diff(new_doc: &Value, old_doc: Option<&Value>) -> HashSet<String> {
    let mut changes = HashSet::new();
    find_changes("", new_doc, old_doc, &mut changes);
    changes
}

fn find_changes(
    path: &str,
    new_val: &Value,
    old_val: Option<&Value>,
    changes: &mut HashSet<String>,
) {
    if let Some(old) = old_val {
        if new_val == old {
            return;
        }
    }

    if !path.is_empty() {
        changes.insert(path.to_string());
    }

    match (new_val, old_val) {
        (Value::Object(new_map), Some(Value::Object(old_map))) => {
            for (k, v) in new_map {
                let new_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path, k)
                };
                find_changes(&new_path, v, old_map.get(k), changes);
            }
        }
        (Value::Object(new_map), None) => {
            for (k, v) in new_map {
                let new_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path, k)
                };
                find_changes(&new_path, v, None, changes);
            }
        }
        _ => {}
    }
}

fn set_value_by_path(doc: &mut Value, path: &str, value: Value) -> bool {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = doc;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                let old_val = obj.get(*part);
                if old_val != Some(&value) {
                    obj.insert(part.to_string(), value);
                    return true;
                }
                return false;
            } else {
                return false;
            }
        } else {
            if !current.is_object() {
                *current = json!({});
            }
            if current.get(*part).is_none() {
                current
                    .as_object_mut()
                    .unwrap()
                    .insert(part.to_string(), json!({}));
            }
            current = current.get_mut(*part).unwrap();
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use tempfile::tempdir;

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[test]
    fn test_manager_get_document_integration() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_test", "db_test");

        // Utilisation de insert_raw pour créer la collection et le doc
        let doc = json!({ "id": "user_123", "name": "Test User" });
        manager.insert_raw("users", &doc).unwrap();

        let result = manager.get_document("users", "user_123").unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["name"], "Test User");

        let missing = manager.get_document("users", "ghost").unwrap();
        assert!(missing.is_none());
    }

    #[test]
    fn test_read_many_parallel() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_test", "db_test");

        // Création de 100 documents
        for i in 0..100 {
            let doc = json!({ "id": i.to_string(), "val": i });
            manager.insert_raw("items", &doc).unwrap();
        }

        // Lecture parallèle de 10 items dispersés
        let ids: Vec<String> = vec!["10", "20", "50", "80", "99"]
            .into_iter()
            .map(String::from)
            .collect();
        let results = manager.read_many("items", &ids).unwrap();

        assert_eq!(results.len(), 5);
        // Vérification qu'on a bien récupéré les bons IDs
        for res in results {
            let id = res["id"].as_str().unwrap();
            assert!(ids.contains(&id.to_string()));
        }
    }

    #[test]
    fn test_read_many_strict_integrity() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_test", "db_test");

        manager
            .insert_raw("items", &json!({ "id": "1", "val": "A" }))
            .unwrap();

        // Requête avec un ID qui n'existe pas -> Doit échouer en mode strict
        let ids = vec!["1".to_string(), "999".to_string()];
        let result = manager.read_many("items", &ids);

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("DATABASE CORRUPTION"));
    }
}
