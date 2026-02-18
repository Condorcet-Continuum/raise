// FICHIER : src-tauri/src/json_db/collections/manager.rs

use crate::json_db::indexes::IndexManager;
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::{file_storage, StorageEngine};
use crate::rules_engine::{EvalError, Evaluator, Rule, RuleStore};

use super::collection;
use super::data_provider::CachedDataProvider;

use crate::utils::config::AppConfig;
use crate::utils::data::{self, HashSet};
use crate::utils::io;
use crate::utils::prelude::*;

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

    pub async fn init_db(&self) -> Result<bool> {
        let created = file_storage::create_db(&self.storage.config, &self.space, &self.db).await?;
        if created {
            self.ensure_system_index().await?;
        }
        Ok(created)
    }

    pub async fn drop_db(&self) -> Result<bool> {
        let db_path = self.storage.config.db_root(&self.space, &self.db);
        if !db_path.exists() {
            return Ok(false);
        }
        file_storage::drop_db(
            &self.storage.config,
            &self.space,
            &self.db,
            file_storage::DropMode::Hard,
        )
        .await?;
        Ok(true)
    }

    // --- HELPER DE RÉSOLUTION D'URI (HIÉRARCHIQUE) ---

    /// Détermine la meilleure URI pour un schéma donné en suivant la hiérarchie de configuration.
    /// Ordre : Local > User > Workstation > System
    fn resolve_best_schema_uri(&self, input_path_or_uri: &str) -> String {
        // Nettoyage du chemin relatif
        let relative_path = if let Some(idx) = input_path_or_uri.find("/schemas/v1/") {
            &input_path_or_uri[idx + "/schemas/v1/".len()..]
        } else {
            input_path_or_uri
        };

        // 1. PRIORITÉ ABSOLUE : Local (Arguments de la DB courante)
        let local_path = self
            .storage
            .config
            .db_schemas_root(&self.space, &self.db)
            .join("v1")
            .join(relative_path);

        if local_path.exists() {
            return format!(
                "db://{}/{}/schemas/v1/{}",
                self.space, self.db, relative_path
            );
        }

        let app_config = AppConfig::get();

        // 2. PRIORITÉ USER : Config Utilisateur
        if let Some(user_cfg) = &app_config.user {
            if let (Some(u_domain), Some(u_db)) = (&user_cfg.default_domain, &user_cfg.default_db) {
                let user_path = self
                    .storage
                    .config
                    .db_schemas_root(u_domain, u_db)
                    .join("v1")
                    .join(relative_path);

                if user_path.exists() {
                    return format!("db://{}/{}/schemas/v1/{}", u_domain, u_db, relative_path);
                }
            }
        }

        // 3. PRIORITÉ WORKSTATION : Config Poste
        if let Some(ws_cfg) = &app_config.workstation {
            if let (Some(w_domain), Some(w_db)) = (&ws_cfg.default_domain, &ws_cfg.default_db) {
                let ws_path = self
                    .storage
                    .config
                    .db_schemas_root(w_domain, w_db)
                    .join("v1")
                    .join(relative_path);

                if ws_path.exists() {
                    return format!("db://{}/{}/schemas/v1/{}", w_domain, w_db, relative_path);
                }
            }
        }

        // 4. PRIORITÉ SYSTEM CONFIG : Config Globale
        let sys_domain = &app_config.system_domain;
        let sys_db = &app_config.system_db;

        let sys_path = self
            .storage
            .config
            .db_schemas_root(sys_domain, sys_db)
            .join("v1")
            .join(relative_path);

        if sys_path.exists() {
            return format!(
                "db://{}/{}/schemas/v1/{}",
                sys_domain, sys_db, relative_path
            );
        }

        // 5. FALLBACK ULTIME : _system/_system (Si la config système pointe ailleurs par erreur)
        if sys_domain != "_system" || sys_db != "_system" {
            let hard_sys_path = self
                .storage
                .config
                .db_schemas_root("_system", "_system")
                .join("v1")
                .join(relative_path);

            if hard_sys_path.exists() {
                return format!("db://_system/_system/schemas/v1/{}", relative_path);
            }
        }

        // Défaut : On retourne l'URI locale pour générer une erreur explicite "Introuvable"
        format!(
            "db://{}/{}/schemas/v1/{}",
            self.space, self.db, relative_path
        )
    }

    /// Charge le registre approprié en fonction de l'URI.
    async fn get_registry_for_uri(&self, uri: &str) -> Result<SchemaRegistry> {
        let app_config = AppConfig::get();
        let sys_domain = &app_config.system_domain;
        let sys_db = &app_config.system_db;

        let sys_prefix_config = format!("db://{}/{}/", sys_domain, sys_db);
        let sys_prefix_hard = "db://_system/_system/";

        if uri.starts_with(&sys_prefix_config) {
            SchemaRegistry::from_db(&self.storage.config, sys_domain, sys_db).await
        } else if uri.starts_with(sys_prefix_hard) {
            SchemaRegistry::from_db(&self.storage.config, "_system", "_system").await
        } else {
            // Note : Pour les cas User/Workstation, le chargement local suffit souvent
            // car le registre est capable de charger d'autres domaines si nécessaire.
            SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db).await
        }
    }

    // --- MÉTHODES DE LECTURE ---

    pub async fn get_document(&self, collection: &str, id: &str) -> Result<Option<Value>> {
        self.storage
            .read_document(&self.space, &self.db, collection, id)
            .await
    }

    pub async fn get(&self, collection: &str, id: &str) -> Result<Option<Value>> {
        self.get_document(collection, id).await
    }

    pub async fn read_many(&self, collection: &str, ids: &[String]) -> Result<Vec<Value>> {
        let mut docs = Vec::with_capacity(ids.len());
        for id in ids {
            let doc_opt = self
                .get_document(collection, id)
                .await
                .map_err(|e| AppError::Database(format!("Erreur I/O lecture ID {}: {}", id, e)))?;

            match doc_opt {
                Some(doc) => docs.push(doc),
                None =>  return Err(AppError::Database(format!(
                    "DATABASE CORRUPTION: L'index pointe vers l'ID '{}' mais le fichier est introuvable dans '{}'", 
                    id, collection
                ))),
            }
        }
        Ok(docs)
    }

    pub async fn list_all(&self, collection: &str) -> Result<Vec<Value>> {
        collection::list_documents(&self.storage.config, &self.space, &self.db, collection).await
    }

    pub async fn list_collections(&self) -> Result<Vec<String>> {
        collection::list_collection_names_fs(&self.storage.config, &self.space, &self.db).await
    }

    // --- GESTION INDEX SYSTÈME ---
    pub async fn ensure_system_index(&self) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        let mut system_doc = if sys_path.exists() {
            io::read_json(&sys_path).await?
        } else {
            json!({
                "space": self.space,
                "database": self.db,
                "version": 1,
                "collections": {}
            })
        };

        self.save_system_index(&mut system_doc).await
    }

    async fn save_system_index(&self, doc: &mut Value) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        // ✅ Utilisation du helper robuste pour l'index lui-même
        let expected_uri = self.resolve_best_schema_uri("db/index.schema.json");

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

        let reg = self.get_registry_for_uri(&expected_uri).await?;

        if let Ok(validator) = SchemaValidator::compile_with_registry(&expected_uri, &reg) {
            if let Err(e) = validator.compute_then_validate(doc) {
                warn!("⚠️ Index système invalide (sauvegarde forcée): {}", e);
            }
        }

        io::write_json_atomic(&sys_path, doc).await?;
        Ok(())
    }

    // --- GESTION DES COLLECTIONS ---

    pub async fn create_collection(&self, name: &str, schema_uri: Option<String>) -> Result<()> {
        if !self.storage.config.db_root(&self.space, &self.db).exists() {
            self.init_db().await?;
        }

        let final_schema_uri = if let Some(uri) = schema_uri {
            // ✅ Utilisation du helper robuste pour le schéma de collection
            self.resolve_best_schema_uri(&uri)
        } else {
            self.resolve_schema_from_index(name)
                .await
                .unwrap_or_default()
        };

        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, name);
        if !col_path.exists() {
            io::ensure_dir(&col_path).await?;
        }

        let meta = json!({ "schema": final_schema_uri, "indexes": [] });
        let meta_path = col_path.join("_meta.json");

        io::write_json_atomic(&meta_path, &meta).await?;

        self.update_system_index_collection(name, &final_schema_uri)
            .await?;
        Ok(())
    }

    pub async fn drop_collection(&self, name: &str) -> Result<()> {
        collection::drop_collection(&self.storage.config, &self.space, &self.db, name).await?;
        self.remove_collection_from_system_index(name).await?;
        Ok(())
    }

    // --- INDEXES SECONDAIRES ---
    pub async fn create_index(&self, collection: &str, field: &str, kind: &str) -> Result<()> {
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        idx_mgr.create_index(collection, field, kind).await
    }

    pub async fn drop_index(&self, collection: &str, field: &str) -> Result<()> {
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        idx_mgr.drop_index(collection, field).await
    }

    // --- HELPER INDEX SYSTÈME & RÉSOLUTION SCHÉMA ---

    async fn resolve_schema_from_index(&self, col_name: &str) -> Result<String> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        if !sys_path.exists() {
            return Err(AppError::Database(
                "Index _system.json introuvable".to_string(),
            ));
        }
        let sys_json: Value = io::read_json(&sys_path).await?;
        let ptr = format!("/collections/{}/schema", col_name);

        let raw_path = sys_json
            .pointer(&ptr)
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Database(format!("Collection '{}' inconnue", col_name)))?;

        if raw_path.is_empty() {
            return Ok(String::new());
        }

        // ✅ Utilisation du helper robuste ici aussi
        Ok(self.resolve_best_schema_uri(raw_path))
    }

    async fn update_system_index_collection(&self, col_name: &str, schema_uri: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        let mut system_doc = if sys_path.exists() {
            io::read_json(&sys_path).await?
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
        self.save_system_index(&mut system_doc).await?;
        Ok(())
    }

    async fn remove_collection_from_system_index(&self, col_name: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        if !sys_path.exists() {
            return Ok(());
        }
        let content = io::read_to_string(&sys_path).await?;
        let mut system_doc: Value = data::parse(&content)?;
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
            self.save_system_index(&mut system_doc).await?;
        }
        Ok(())
    }

    async fn add_item_to_index(&self, col_name: &str, id: &str) -> Result<()> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");
        let mut system_doc = if sys_path.exists() {
            io::read_json(&sys_path).await?
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
                    .await
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
        self.save_system_index(&mut system_doc).await?;
        Ok(())
    }

    // --- ÉCRITURE ET MISE À JOUR ---

    pub async fn insert_raw(&self, collection: &str, doc: &Value) -> Result<()> {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AppError::Validation("ID manquant dans le document".to_string()))?;
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
            self.create_collection(collection, schema_hint).await?;
        }
        self.storage
            .write_document(&self.space, &self.db, collection, id, doc)
            .await?;
        self.add_item_to_index(collection, id).await?;
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        if let Err(_e) = idx_mgr.index_document(collection, doc).await {
            #[cfg(debug_assertions)]
            warn!("⚠️ Indexation secondaire échouée: {}", _e);
        }
        Ok(())
    }

    pub async fn insert_with_schema(&self, collection: &str, mut doc: Value) -> Result<Value> {
        self.prepare_document(collection, &mut doc).await?;
        self.insert_raw(collection, &doc).await?;
        Ok(doc)
    }

    pub async fn update_document(
        &self,
        collection: &str,
        id: &str,
        patch_data: Value,
    ) -> Result<Value> {
        let old_doc_opt = self.get_document(collection, id).await?;
        let mut doc = old_doc_opt
            .ok_or_else(|| AppError::Database("Document introuvable pour update".to_string()))?;

        json_merge(&mut doc, patch_data);

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("id".to_string(), Value::String(id.to_string()));
            let now = Utc::now().to_rfc3339();
            obj.insert("updatedAt".to_string(), Value::String(now));
        }

        self.prepare_document(collection, &mut doc).await?;

        self.storage
            .write_document(&self.space, &self.db, collection, id, &doc)
            .await?;

        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        let _ = idx_mgr.index_document(collection, &doc).await;

        Ok(doc)
    }

    pub async fn upsert_document(&self, collection: &str, data: Value) -> Result<String> {
        let id_opt = data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut target_id = None;

        if let Some(ref id) = id_opt {
            if let Ok(Some(_)) = self.get_document(collection, id).await {
                target_id = Some(id.clone());
            }
        }

        match target_id {
            Some(id) => {
                self.update_document(collection, &id, data).await?;
                Ok(format!("Updated: {}", id))
            }
            None => {
                let doc = self.insert_with_schema(collection, data).await?;
                let new_id = doc.get("id").and_then(|v| v.as_str()).unwrap().to_string();
                Ok(format!("Created: {}", new_id))
            }
        }
    }

    pub async fn delete_document(&self, collection: &str, id: &str) -> Result<bool> {
        let old_doc = self.get_document(collection, id).await?;
        self.storage
            .delete_document(&self.space, &self.db, collection, id)
            .await?;
        if let Some(doc) = old_doc {
            let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
            let _ = idx_mgr.remove_document(collection, &doc).await;
        }
        Ok(true)
    }

    pub async fn prepare_document(&self, collection: &str, doc: &mut Value) -> Result<()> {
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

        let mut resolved_uri = None;

        if meta_path.exists() {
            if let Ok(content) = io::read_to_string(&meta_path).await {
                if let Ok(meta) = data::parse::<Value>(&content) {
                    if let Some(s) = meta.get("schema").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            // ✅ Normalisation via le helper robuste
                            resolved_uri = Some(self.resolve_best_schema_uri(s));
                        }
                    }
                }
            }
        }

        if resolved_uri.is_none() {
            if let Ok(sys_uri) = self.resolve_schema_from_index(collection).await {
                if !sys_uri.is_empty() {
                    resolved_uri = Some(sys_uri);
                }
            }
        }

        #[cfg(debug_assertions)]
        println!(
            "🔧 Prepare Doc [{}]: Schema URI résolu = {:?}",
            collection, resolved_uri
        );

        if let Some(uri) = &resolved_uri {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("$schema".to_string(), Value::String(uri.clone()));
            }

            let reg = self.get_registry_for_uri(uri).await?;

            if let Err(e) = apply_business_rules(self, collection, doc, None, &reg, uri).await {
                warn!("⚠️ Erreur règles métier (non bloquant): {}", e);
            }

            let validator = SchemaValidator::compile_with_registry(uri, &reg)?;
            validator.compute_then_validate(doc)?;
        } else {
            #[cfg(debug_assertions)]
            warn!(
                "⚠️ ATTENTION: Aucun schéma trouvé pour la collection '{}'. Insertion schemaless.",
                collection
            );
        }

        self.apply_semantic_logic(doc, resolved_uri.as_deref())
            .map_err(|e| AppError::Validation(format!("Validation sémantique échouée: {}", e)))?;
        Ok(())
    }

    // --- OPTIMISATION SEMANTIQUE ---
    fn apply_semantic_logic(&self, doc: &mut Value, schema_uri: Option<&str>) -> Result<()> {
        let layer_hint = if let Some(uri) = schema_uri {
            if uri.contains("/oa/") {
                Some("oa")
            } else if uri.contains("/sa/") {
                Some("sa")
            } else if uri.contains("/la/") {
                Some("la")
            } else if uri.contains("/pa/") {
                Some("pa")
            } else if uri.contains("/epbs/") {
                Some("epbs")
            } else if uri.contains("/data/") {
                Some("data")
            } else {
                None
            }
        } else {
            None
        };

        if let Some(obj) = doc.as_object_mut() {
            if !obj.contains_key("@context") {
                let registry = VocabularyRegistry::global();

                if let Some(layer) = layer_hint {
                    if let Some(layer_ctx) = registry.get_context_for_layer(layer) {
                        obj.insert("@context".to_string(), layer_ctx);
                    } else {
                        let defaults = registry.get_default_context();
                        if let Ok(val) = data::to_value(defaults) {
                            obj.insert("@context".to_string(), val);
                        }
                    }
                } else {
                    let defaults = registry.get_default_context();
                    if let Ok(val) = data::to_value(defaults) {
                        obj.insert("@context".to_string(), val);
                    }
                }
            }
        }

        let has_type = doc.get("@type").is_some()
            || doc
                .get("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")
                .is_some();

        if has_type {
            let processor = JsonLdProcessor::new()
                .with_doc_context(doc)
                .unwrap_or_else(|_| JsonLdProcessor::new());

            if let Some(type_uri) = processor.get_type(doc) {
                let registry = VocabularyRegistry::global();
                let mut expanded_type = processor.context_manager().expand_term(&type_uri);

                if !VocabularyRegistry::is_iri(&expanded_type) && expanded_type.contains(':') {
                    let deep_expanded = processor.context_manager().expand_term(&expanded_type);
                    if VocabularyRegistry::is_iri(&deep_expanded) {
                        expanded_type = deep_expanded;
                    }
                }

                if !registry.has_class(&expanded_type) {
                    #[cfg(debug_assertions)]
                    println!(
                        "⚠️ [Semantic Warning] Type inconnu: {} (Expanded from {})",
                        expanded_type, type_uri
                    );
                }
            }
        }
        Ok(())
    }
}

fn json_merge(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                json_merge(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn apply_business_rules(
    manager: &CollectionsManager<'_>,
    collection_name: &str,
    doc: &mut Value,
    old_doc: Option<&Value>,
    registry: &SchemaRegistry,
    schema_uri: &str,
) -> Result<()> {
    let mut store = RuleStore::new(manager);

    if let Err(e) = store.sync_from_db().await {
        warn!(
            "⚠️ Warning: Impossible de charger les règles système: {}",
            e
        );
    }

    if let Some(schema) = registry.get_by_uri(schema_uri) {
        if let Some(rules_array) = schema.get("x_rules").and_then(|v| v.as_array()) {
            for (index, rule_val) in rules_array.iter().enumerate() {
                match data::from_value::<Rule>(rule_val.clone()) {
                    Ok(rule) => {
                        if let Err(e) = store.register_rule(collection_name, rule).await {
                            warn!("⚠️ Erreur enregistrement règle (index {}): {}", index, e);
                        }
                    }
                    Err(e) => {
                        warn!("⚠️ Règle invalide dans le schéma (index {}): {}", index, e)
                    }
                }
            }
        }
    }

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
            match Evaluator::evaluate(&rule.expr, doc, &provider).await {
                Ok(result) => {
                    if set_value_by_path(doc, &rule.target, result.into_owned()) {
                        next_changes.insert(rule.target.clone());
                    }
                }
                Err(EvalError::VarNotFound(_)) => continue,
                Err(e) => {
                    return Err(AppError::Database(format!(
                        "Erreur calcul règle '{}': {}",
                        rule.id, e
                    )))
                }
            }
        }
        current_changes = next_changes;
        passes += 1;
    }
    Ok(())
}

fn compute_diff(new_doc: &Value, old_doc: Option<&Value>) -> data::HashSet<String> {
    let mut changes = data::HashSet::new();
    find_changes("", new_doc, old_doc, &mut changes);
    changes
}

fn find_changes(
    path: &str,
    new_val: &Value,
    old_val: Option<&Value>,
    changes: &mut data::HashSet<String>,
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
    use crate::utils::io::tempdir;

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        crate::utils::config::test_mocks::inject_mock_config();

        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[tokio::test]
    async fn test_manager_get_document_integration() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_test", "db_test");
        manager.init_db().await.unwrap();

        let doc = json!({ "id": "user_123", "name": "Test User" });
        manager.insert_raw("users", &doc).await.unwrap();

        let result = manager.get_document("users", "user_123").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["name"], "Test User");

        let missing = manager.get_document("users", "ghost").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_read_many_parallel() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_test", "db_test");
        manager.init_db().await.unwrap();

        for i in 0..100 {
            let doc = json!({ "id": i.to_string(), "val": i });
            manager.insert_raw("items", &doc).await.unwrap();
        }

        let ids: Vec<String> = vec!["10", "20", "50", "80", "99"]
            .into_iter()
            .map(String::from)
            .collect();
        let results = manager.read_many("items", &ids).await.unwrap();

        assert_eq!(results.len(), 5);
        for res in results {
            let id = res["id"].as_str().unwrap();
            assert!(ids.contains(&id.to_string()));
        }
    }

    #[tokio::test]
    async fn test_read_many_strict_integrity() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_test", "db_test");
        manager.init_db().await.unwrap();

        manager
            .insert_raw("items", &json!({ "id": "1", "val": "A" }))
            .await
            .unwrap();

        let ids = vec!["1".to_string(), "999".to_string()];
        let result = manager.read_many("items", &ids).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("DATABASE CORRUPTION"));
    }

    #[tokio::test]
    async fn test_crud_workflow() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let mgr = CollectionsManager::new(&storage, "test", "crud");
        mgr.init_db().await.unwrap();

        mgr.create_collection("items", None).await.unwrap();

        // 1. CREATE (Insert)
        let doc = json!({ "name": "Item 1", "price": 100 });
        let created_doc = mgr.insert_with_schema("items", doc).await.unwrap();
        let id = created_doc["id"].as_str().unwrap().to_string();

        // Vérif existence
        let fetched = mgr.get_document("items", &id).await.unwrap();
        assert!(fetched.is_some());

        // 2. UPDATE (Partial Merge)
        mgr.update_document("items", &id, json!({ "price": 150, "status": "active" }))
            .await
            .unwrap();

        let updated = mgr.get_document("items", &id).await.unwrap().unwrap();
        assert_eq!(updated["price"], 150);
        assert_eq!(updated["name"], "Item 1"); // Champ préservé
        assert_eq!(updated["status"], "active"); // Champ ajouté

        // 3. DELETE
        let deleted = mgr.delete_document("items", &id).await.unwrap();
        assert!(deleted);

        let missing = mgr.get_document("items", &id).await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_upsert_idempotence() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let mgr = CollectionsManager::new(&storage, "test", "upsert");
        mgr.init_db().await.unwrap();

        mgr.create_collection("configs", None).await.unwrap();

        // 1. Premier Upsert (Création)
        let data1 = json!({ "id": "config-01", "val": "A" });
        let res1 = mgr.upsert_document("configs", data1).await.unwrap();
        assert!(res1.contains("Created"));

        // 2. Deuxième Upsert (Mise à jour)
        let data2 = json!({ "id": "config-01", "val": "B" });
        let res2 = mgr.upsert_document("configs", data2).await.unwrap();
        assert!(res2.contains("Updated"));

        // Vérif finale
        let final_doc = mgr
            .get_document("configs", "config-01")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(final_doc["val"], "B");
        assert_eq!(final_doc["id"], "config-01");
    }
}
