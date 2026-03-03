// FICHIER : src-tauri/src/json_db/collections/manager.rs

use crate::json_db::indexes::IndexManager;
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::{file_storage, StorageEngine};
use crate::rules_engine::{Evaluator, Rule, RuleStore};
use crate::utils::{Future, Pin};

use super::collection;
use super::data_provider::CachedDataProvider;

use crate::utils::config::AppConfig;
use crate::utils::data::{self, HashSet};
use crate::utils::io;
use crate::utils::prelude::*;

pub enum EntityIdentity {
    Id(String),
    Name(String),
}

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

    pub async fn init_db(&self) -> RaiseResult<bool> {
        let expected_uri = "db://_system/_system/schemas/v1/db/index.schema.json";
        let reg = self.get_registry_for_uri(expected_uri).await?;

        if reg.get_by_uri(expected_uri).is_none() {
            raise_error!(
                "ERR_DB_MISSING_CORE_SCHEMA",
                error = "Le schéma JSON 'index.schema.json' est introuvable. Initialisation impossible.",
                context = json!({ "required_schema": expected_uri })
            );
        }

        let existing_doc =
            file_storage::read_system_index(&self.storage.config, &self.space, &self.db).await?;
        let is_new = existing_doc.is_none();

        let mut system_doc = existing_doc.unwrap_or_else(|| {
            json!({
                "$schema": expected_uri,
                "name": format!("{}_{}", self.space, self.db),
                "space": self.space,
                "database": self.db
            })
        });

        let validator = SchemaValidator::compile_with_registry(expected_uri, &reg)?;
        if let Err(e) = validator.compute_then_validate(&mut system_doc) {
            raise_error!(
                "ERR_DB_INDEX_VALIDATION_FAILED",
                error = e,
                context = json!({ "action": "init_db_instantiation" })
            );
        }

        // 🎯 L'APPEL UNIQUE ET MAGIQUE : create_db s'occupe de tout le volet physique
        let created =
            file_storage::create_db(&self.storage.config, &self.space, &self.db, &system_doc)
                .await?;

        if is_new || created {
            file_storage::write_system_index(
                &self.storage.config,
                &self.space,
                &self.db,
                &system_doc,
            )
            .await?;
        }

        Ok(created || is_new)
    }

    pub async fn drop_db(&self) -> RaiseResult<bool> {
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

    pub async fn load_index(&self) -> RaiseResult<Value> {
        let sys_path = self
            .storage
            .config
            .db_root(&self.space, &self.db)
            .join("_system.json");

        if !sys_path.exists() {
            // 🎯 ERREUR SYSTÉMATIQUE : Pas de permissivité.
            raise_error!(
                "ERR_DB_SYSTEM_INDEX_NOT_FOUND",
                error = "Opération refusée : l'index de la base de données est introuvable. La base doit être explicitement initialisée via init_db().",
                context = json!({
                    "path": sys_path.to_string_lossy().to_string(), 
                    "db": self.db,
                    "space": self.space
                })
            );
        }

        io::read_json(&sys_path).await
    }
    // --- HELPER DE RÉSOLUTION D'URI (HIÉRARCHIQUE) ---
    fn resolve_best_schema_uri(&self, input_path_or_uri: &str) -> String {
        let relative_path = if let Some(idx) = input_path_or_uri.find("/schemas/v1/") {
            &input_path_or_uri[idx + "/schemas/v1/".len()..]
        } else {
            input_path_or_uri
        };

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

        format!(
            "db://{}/{}/schemas/v1/{}",
            self.space, self.db, relative_path
        )
    }

    async fn get_registry_for_uri(&self, uri: &str) -> RaiseResult<SchemaRegistry> {
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
            SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db).await
        }
    }

    // ============================================================================
    // GESTION DES SCHÉMAS (DDL)
    // ============================================================================

    /// Construit l'URI standardisée en utilisant le space et la db du manager
    fn build_schema_uri(&self, schema_name: &str) -> String {
        if schema_name.starts_with("db://") {
            return schema_name.to_string();
        }
        let clean_path = schema_name
            .trim_start_matches('/')
            .trim_start_matches("schemas/")
            .trim_start_matches("v1/")
            .trim_start_matches('/');
        format!("db://{}/{}/schemas/v1/{}", self.space, self.db, clean_path)
    }

    pub async fn create_schema_def(&self, schema_name: &str, schema: Value) -> RaiseResult<()> {
        let uri = self.build_schema_uri(schema_name);
        let mut reg = self.get_registry_for_uri(&uri).await?;
        reg.create_schema(&uri, schema).await
    }

    pub async fn drop_schema_def(&self, schema_name: &str) -> RaiseResult<()> {
        let uri = self.build_schema_uri(schema_name);
        let mut reg = self.get_registry_for_uri(&uri).await?;
        reg.drop_schema(&uri).await
    }

    pub async fn add_schema_property(
        &self,
        schema_name: &str,
        prop_name: &str,
        prop_def: Value,
    ) -> RaiseResult<()> {
        let uri = self.build_schema_uri(schema_name);
        let mut reg = self.get_registry_for_uri(&uri).await?;
        reg.add_property(&uri, prop_name, prop_def).await
    }

    pub async fn alter_schema_property(
        &self,
        schema_name: &str,
        prop_name: &str,
        prop_def: Value,
    ) -> RaiseResult<()> {
        let uri = self.build_schema_uri(schema_name);
        let mut reg = self.get_registry_for_uri(&uri).await?;
        reg.alter_property(&uri, prop_name, prop_def).await
    }

    pub async fn drop_schema_property(
        &self,
        schema_name: &str,
        prop_name: &str,
    ) -> RaiseResult<()> {
        let uri = self.build_schema_uri(schema_name);
        let mut reg = self.get_registry_for_uri(&uri).await?;
        reg.drop_property(&uri, prop_name).await
    }

    pub async fn list_schemas(&self) -> RaiseResult<Vec<String>> {
        // On charge le registre complet pour l'espace et la base de données actuels
        let reg = SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db).await?;

        // On récupère toutes les URIs et on les trie par ordre alphabétique
        let mut uris = reg.list_uris();
        uris.sort();

        Ok(uris)
    }
    // --- MÉTHODES DE LECTURE ---

    pub async fn get_document(&self, collection: &str, id: &str) -> RaiseResult<Option<Value>> {
        self.storage
            .read_document(&self.space, &self.db, collection, id)
            .await
    }

    pub async fn get(&self, collection: &str, id: &str) -> RaiseResult<Option<Value>> {
        self.get_document(collection, id).await
    }

    pub async fn read_many(&self, collection: &str, ids: &[String]) -> RaiseResult<Vec<Value>> {
        let mut docs = Vec::with_capacity(ids.len());
        for id in ids {
            let doc_opt = match self.get_document(collection, id).await {
                Ok(doc) => doc,
                Err(e) => raise_error!(
                    "ERR_DB_DOCUMENT_READ",
                    error = e,
                    context = json!({
                        "collection": collection,
                        "document_id": id
                    })
                ),
            };
            let Some(doc) = doc_opt else {
                raise_error!(
                    "ERR_DB_CORRUPTION_INDEX_MISMATCH",
                    error = "Document indexé mais introuvable physiquement",
                    context = json!({ "id": id, "coll": collection })
                );
            };
            docs.push(doc);
        }
        Ok(docs)
    }

    pub async fn list_all(&self, collection: &str) -> RaiseResult<Vec<Value>> {
        // ✅ CORRECTION : On passe self.storage au lieu de &self.storage.config
        // Cela permet à list_documents de taper dans le cache.
        collection::list_documents(self.storage, &self.space, &self.db, collection).await
    }

    pub async fn list_collections(&self) -> RaiseResult<Vec<String>> {
        // Reste inchangé car c'est une lecture de métadonnées de dossier
        collection::list_collection_names_fs(&self.storage.config, &self.space, &self.db).await
    }

    async fn save_system_index(&self, doc: &mut Value) -> RaiseResult<()> {
        let expected_uri = "db://_system/_system/schemas/v1/db/index.schema.json".to_string();

        if let Some(obj) = doc.as_object_mut() {
            obj.insert("$schema".to_string(), Value::String(expected_uri.clone()));
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
        if reg.get_by_uri(&expected_uri).is_none() {
            raise_error!(
                "ERR_DB_SECURITY_VIOLATION",
                error = "Schéma d'index système introuvable ou non autorisé.",
                context = json!({
                    "required_uri": expected_uri,
                    "action": "enforce_system_integrity",
                    "hint": "Le schéma 'index.schema.json' doit impérativement résider dans '_system/_system/schemas/v1/db'."
                })
            );
        }
        if let Ok(validator) = SchemaValidator::compile_with_registry(&expected_uri, &reg) {
            if let Err(e) = validator.compute_then_validate(doc) {
                warn!("⚠️ Index système invalide (sauvegarde forcée): {}", e);
            }
        }

        file_storage::write_system_index(&self.storage.config, &self.space, &self.db, doc).await?;
        Ok(())
    }

    // --- GESTION DES COLLECTIONS ---
    pub async fn create_collection(
        &self,
        name: &str,
        schema_uri: Option<String>,
    ) -> RaiseResult<()> {
        if !self.storage.config.db_root(&self.space, &self.db).exists() {
            self.init_db().await?;
        }

        let final_schema_uri = if let Some(uri) = schema_uri {
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

    pub async fn drop_collection(&self, name: &str) -> RaiseResult<()> {
        collection::drop_collection(&self.storage.config, &self.space, &self.db, name).await?;
        self.remove_collection_from_system_index(name).await?;
        Ok(())
    }

    // --- INDEXES SECONDAIRES ---
    pub async fn create_index(&self, collection: &str, field: &str, kind: &str) -> RaiseResult<()> {
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        idx_mgr.create_index(collection, field, kind).await
    }

    pub async fn drop_index(&self, collection: &str, field: &str) -> RaiseResult<()> {
        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        idx_mgr.drop_index(collection, field).await
    }

    // --- HELPER INDEX SYSTÈME & RÉSOLUTION SCHÉMA ---

    async fn resolve_schema_from_index(&self, col_name: &str) -> RaiseResult<String> {
        let sys_json = self.load_index().await?;
        // 1. 🎯 Vérification de l'intégrité du schéma de l'index (Strict Trust Root)
        let current_schema = sys_json
            .get("$schema")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if !current_schema.contains("_system/_system/schemas/v1/db/") {
            raise_error!(
                "ERR_DB_INTEGRITY_COMPROMISED",
                error = "L'index de la base utilise un schéma non certifié ou hors du noyau de confiance.",
                context = json!({ "found_schema": current_schema })
            );
        }

        // 2. 🎯 RECHERCHE BI-MODALE (Collections OU Rules)
        // On cherche d'abord dans les collections de données, puis dans les collections de règles
        let col_ptr = format!("/collections/{}/schema", col_name);
        let rule_ptr = format!("/rules/{}/schema", col_name);

        let raw_path = sys_json
            .pointer(&col_ptr)
            .or_else(|| sys_json.pointer(&rule_ptr)) // Si pas trouvé dans collections, regarde dans rules
            .and_then(|v| v.as_str());

        let Some(path) = raw_path else {
            raise_error!(
                "ERR_DB_COLLECTION_NOT_FOUND",
                error = format!(
                    "La cible '{}' est inconnue dans les sections collections ou rules de l'index.",
                    col_name
                ),
                context = json!({
                    "target_name": col_name,
                    "searched_paths": [col_ptr, rule_ptr]
                })
            );
        };

        if path.is_empty() {
            return Ok(String::new());
        }

        // 3. Résolution de l'URI finale (db://...)
        Ok(self.resolve_best_schema_uri(path))
    }

    async fn update_system_index_collection(
        &self,
        col_name: &str,
        schema_uri: &str,
    ) -> RaiseResult<()> {
        let mut system_doc = self.load_index().await?; // 🎯 Douane stricte

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

    async fn remove_collection_from_system_index(&self, col_name: &str) -> RaiseResult<()> {
        let mut system_doc = self.load_index().await?;
        let mut changed = false;

        // 2. Logique de suppression
        if let Some(cols) = system_doc
            .get_mut("collections")
            .and_then(|c| c.as_object_mut())
        {
            if cols.remove(col_name).is_some() {
                changed = true;
            }
        }

        // 3. Sauvegarde centralisée uniquement en cas de modification
        if changed {
            self.save_system_index(&mut system_doc).await?;
        }

        Ok(())
    }

    async fn add_item_to_index(&self, col_name: &str, id: &str) -> RaiseResult<()> {
        let mut system_doc = self.load_index().await?; // 🎯 Douane stricte

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

    async fn remove_item_from_index(&self, col_name: &str, id: &str) -> RaiseResult<()> {
        // 🎯 1. Douane stricte : lecture sécurisée de l'index
        let mut system_doc = self.load_index().await?;

        let filename = format!("{}.json", id);
        let mut changed = false;

        // 2. Logique de suppression
        if let Some(cols) = system_doc
            .get_mut("collections")
            .and_then(|c| c.as_object_mut())
        {
            if let Some(col_entry) = cols.get_mut(col_name) {
                if let Some(items) = col_entry.get_mut("items").and_then(|i| i.as_array_mut()) {
                    let initial_len = items.len();

                    // On filtre pour garder tous les items SAUF celui qu'on supprime
                    items.retain(|i| i.get("file").and_then(|f| f.as_str()) != Some(&filename));

                    // Si la taille du tableau a diminué, c'est qu'on a bien supprimé l'item
                    if items.len() < initial_len {
                        changed = true;
                    }
                }
            }
        }

        // 3. Sauvegarde via la méthode centralisée
        if changed {
            self.save_system_index(&mut system_doc).await?;
        }

        Ok(())
    }

    // --- ÉCRITURE ET MISE À JOUR ---
    pub async fn insert_raw(&self, collection: &str, doc: &Value) -> RaiseResult<()> {
        let Some(id) = doc.get("id").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_DB_DOCUMENT_ID_MISSING",
                error = "Attribut 'id' manquant ou format invalide dans le document",
                context = json!({
                    "expected_field": "id",
                    "available_keys": doc.as_object().map(|m| m.keys().collect::<Vec<_>>())
                })
            );
        };
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

    pub async fn insert_with_schema(&self, collection: &str, mut doc: Value) -> RaiseResult<Value> {
        doc = self.resolve_document_references(doc).await?;
        self.prepare_document(collection, &mut doc).await?;
        self.insert_raw(collection, &doc).await?;
        Ok(doc)
    }

    pub async fn update_document(
        &self,
        collection: &str,
        id: &str,
        patch_data: Value,
    ) -> RaiseResult<Value> {
        let resolved_patch = self.resolve_document_references(patch_data).await?;
        let old_doc_opt = self.get_document(collection, id).await?;
        let Some(mut doc) = old_doc_opt else {
            raise_error!(
                "ERR_DB_UPDATE_TARGET_NOT_FOUND",
                error = "Échec de la mise à jour : le document original est introuvable.",
                context = json!({ "action": "update_document" })
            );
        };
        json_merge(&mut doc, resolved_patch);

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

    pub async fn upsert_document(&self, collection: &str, mut data: Value) -> RaiseResult<String> {
        data = self.resolve_document_references(data).await?;

        let id_opt = data
            .get("id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let name_opt = data.get("name").cloned();

        if id_opt.is_none() && name_opt.is_none() {
            raise_error!(
                "ERR_DB_UPSERT_MISSING_IDENTITY",
                error =
                    "Identifiant manquant : l'upsert requiert au moins un champ 'id' ou 'name'.",
                context = json!({
                    "action": "upsert_document",
                    "validation_state": {
                        "has_id": false,
                        "has_name": false
                    },
                    "hint": "Vérifiez que le document JSON contient une clé 'id' ou 'name' à la racine."
                })
            );
        }

        let mut target_id = None;

        if let Some(ref id) = id_opt {
            if let Ok(Some(_)) = self.get_document(collection, id).await {
                target_id = Some(id.clone());
            }
        }

        if target_id.is_none() {
            if let Some(name_val) = name_opt {
                let mut query = Query::new(collection);
                query.filter = Some(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![Condition::eq("name", name_val)],
                });
                query.limit = Some(1);

                let result = QueryEngine::new(self).execute_query(query).await?;

                if let Some(existing_doc) = result.documents.first() {
                    if let Some(found_id) = existing_doc.get("id").and_then(|v| v.as_str()) {
                        target_id = Some(found_id.to_string());
                    }
                }
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

    pub async fn delete_document(&self, collection: &str, id: &str) -> RaiseResult<bool> {
        let old_doc = self.get_document(collection, id).await?;
        self.storage
            .delete_document(&self.space, &self.db, collection, id)
            .await?;
        if let Some(doc) = old_doc {
            let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
            let _ = idx_mgr.remove_document(collection, &doc).await;
        }
        self.remove_item_from_index(collection, id).await?;
        Ok(true)
    }

    pub async fn prepare_document(&self, collection: &str, doc: &mut Value) -> RaiseResult<()> {
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

        if let Err(e) = self.apply_semantic_logic(doc, resolved_uri.as_deref()) {
            raise_error!(
                "ERR_AI_SEMANTIC_VALIDATION_FAIL",
                error = e,
                context = json!({
                    "uri": resolved_uri,
                    "action": "semantic_integrity_check"
                })
            );
        }
        Ok(())
    }

    // --- OPTIMISATION SEMANTIQUE ---
    fn apply_semantic_logic(&self, doc: &mut Value, schema_uri: Option<&str>) -> RaiseResult<()> {
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

    pub async fn delete_identity(
        &self,
        collection: &str,
        identity: EntityIdentity,
    ) -> RaiseResult<()> {
        let target_id = match identity {
            EntityIdentity::Id(id) => id,
            EntityIdentity::Name(name) => {
                let qe = QueryEngine::new(self);
                let mut query = Query::new(collection);
                query.filter = Some(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![Condition::eq("name", crate::utils::json::json!(name))],
                });
                let res = qe.execute_query(query).await?;

                match res.documents.first() {
                    Some(doc) => doc.get("id").and_then(|v| v.as_str()).unwrap().to_string(),
                    None => {
                        raise_error!(
                            "ERR_DB_ENTITY_NOT_FOUND",
                            error = format!(
                                "Aucun document nommé '{}' dans la collection '{}'",
                                name, collection
                            )
                        );
                    }
                }
            }
        };

        self.delete_document(collection, &target_id).await?;

        Ok(())
    }

    pub async fn resolve_document_references(&self, document: Value) -> RaiseResult<Value> {
        resolve_refs_recursive(document, self).await
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
) -> RaiseResult<()> {
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

    // ✅ ANTICIPATION : On passe directement le StorageEngine au lieu du JsonDbConfig !
    let provider = CachedDataProvider::new(manager.storage, &manager.space, &manager.db);
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
                Err(crate::utils::error::AppError::Structured(ref data))
                    if data.code == "ERR_RULE_VAR_NOT_FOUND" =>
                {
                    continue;
                }
                Err(e) => {
                    raise_error!(
                        "ERR_DB_RULE_EVAL_FAIL",
                        context = json!({
                            "rule_id": rule.id,
                            "target_path": rule.target,
                            "evaluator_error": e.to_string(),
                            "action": "apply_document_rules"
                        })
                    );
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

// ============================================================================
// RÉSOLUTION DE RÉFÉRENCES (SMART LINKS)
// ============================================================================

fn parse_smart_link(s: &str) -> Option<(&str, &str, &str)> {
    if !s.starts_with("ref:") {
        return None;
    }
    let parts: Vec<&str> = s.splitn(4, ':').collect();
    if parts.len() == 4 {
        Some((parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

fn resolve_refs_recursive<'a>(
    data: Value,
    col_mgr: &'a CollectionsManager<'a>,
) -> Pin<Box<dyn Future<Output = RaiseResult<Value>> + Send + 'a>> {
    Box::pin(async move {
        match data {
            Value::String(s) => {
                if let Some((col, field, val)) = parse_smart_link(&s) {
                    let mut query = Query::new(col);
                    query.filter = Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: vec![Condition::eq(field, val.into())],
                    });

                    let result = QueryEngine::new(col_mgr).execute_query(query).await?;
                    if let Some(doc) = result.documents.first() {
                        let id = doc
                            .get("id")
                            .or_else(|| doc.get("_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Ok(Value::String(id.to_string()))
                    } else {
                        Ok(Value::String(s))
                    }
                } else {
                    Ok(Value::String(s))
                }
            }
            Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for item in arr {
                    new_arr.push(resolve_refs_recursive(item, col_mgr).await?);
                }
                Ok(Value::Array(new_arr))
            }
            Value::Object(map) => {
                let mut new_map = crate::utils::data::Map::new();
                for (k, v) in map {
                    new_map.insert(k, resolve_refs_recursive(v, col_mgr).await?);
                }
                Ok(Value::Object(new_map))
            }
            _ => Ok(data),
        }
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    // 🎯 Import de notre Sandbox magique !
    use crate::utils::config::test_mocks::DbSandbox;

    // ❌ La fonction setup_env() a été entièrement supprimée !

    #[tokio::test]
    async fn test_manager_get_document_integration() {
        // 1. Initialisation en une ligne
        let sandbox = DbSandbox::new().await;
        // 2. On passe la référence au storage de la sandbox
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
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
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
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
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
        manager.init_db().await.unwrap();

        manager
            .insert_raw("items", &json!({ "id": "1", "val": "A" }))
            .await
            .unwrap();

        let ids = vec!["1".to_string(), "999".to_string()];
        let result = manager.read_many("items", &ids).await;

        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("ERR_DB_CORRUPTION_INDEX_MISMATCH"));
    }

    #[tokio::test]
    async fn test_crud_workflow() {
        let sandbox = DbSandbox::new().await;
        let mgr = CollectionsManager::new(&sandbox.storage, "test", "crud");
        mgr.init_db().await.unwrap();

        mgr.create_collection("items", None).await.unwrap();

        // 1. CREATE (Insert)
        let doc = json!({ "name": "Item 1", "price": 100 });
        let created_doc = mgr.insert_with_schema("items", doc).await.unwrap();
        let id = created_doc["id"].as_str().unwrap().to_string();

        let fetched = mgr.get_document("items", &id).await.unwrap();
        assert!(fetched.is_some());

        // 2. UPDATE
        mgr.update_document("items", &id, json!({ "price": 150, "status": "active" }))
            .await
            .unwrap();

        let updated = mgr.get_document("items", &id).await.unwrap().unwrap();
        assert_eq!(updated["price"], 150);
        assert_eq!(updated["name"], "Item 1");
        assert_eq!(updated["status"], "active");

        // 3. DELETE
        let deleted = mgr.delete_document("items", &id).await.unwrap();
        assert!(deleted);

        let missing = mgr.get_document("items", &id).await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_upsert_idempotence() {
        let sandbox = DbSandbox::new().await;
        let mgr = CollectionsManager::new(&sandbox.storage, "test", "upsert");
        mgr.init_db().await.unwrap();

        mgr.create_collection("configs", None).await.unwrap();

        let data1 = json!({ "id": "config-01", "val": "A" });
        let res1 = mgr.upsert_document("configs", data1).await.unwrap();
        assert!(res1.contains("Created"));

        let data2 = json!({ "id": "config-01", "val": "B" });
        let res2 = mgr.upsert_document("configs", data2).await.unwrap();
        assert!(res2.contains("Updated"));

        let final_doc = mgr
            .get_document("configs", "config-01")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(final_doc["val"], "B");
        assert_eq!(final_doc["id"], "config-01");
    }

    // 🎯 Note: Les tests synchrones restent tels quels car ils ne touchent pas à la DB physique !
    #[test]
    fn test_parse_smart_link_valid() {
        let input = "ref:oa_actors:name:Sécurité";
        let res = super::parse_smart_link(input);
        assert!(res.is_some());
        let (col, field, val) = res.unwrap();
        assert_eq!(col, "oa_actors");
        assert_eq!(field, "name");
        assert_eq!(val, "Sécurité");
    }

    #[test]
    fn test_parse_smart_link_invalid_prefix() {
        assert!(super::parse_smart_link("uuid:1234-5678").is_none());
    }

    #[test]
    fn test_parse_smart_link_missing_parts() {
        assert!(super::parse_smart_link("ref:oa_actors:name").is_none());
    }

    #[test]
    fn test_parse_smart_link_complex_value() {
        let input = "ref:oa_actors:description:Ceci:est:une:description";
        let res = super::parse_smart_link(input);
        assert!(res.is_some());
        let (_col, _field, val) = res.unwrap();
        assert_eq!(val, "Ceci:est:une:description");
    }

    #[tokio::test]
    async fn test_manager_delete_identity() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
        manager.init_db().await.unwrap();

        // On s'assure que la collection existe
        manager.create_collection("users", None).await.unwrap();

        // 1. On prépare deux documents de test
        let doc_alice = json!({ "id": "u_100", "name": "Alice" });
        let doc_bob = json!({ "id": "u_200", "name": "Bob" });

        manager.insert_raw("users", &doc_alice).await.unwrap();
        manager.insert_raw("users", &doc_bob).await.unwrap();

        // Vérification de base : les documents sont bien là
        assert!(manager
            .get_document("users", "u_100")
            .await
            .unwrap()
            .is_some());
        assert!(manager
            .get_document("users", "u_200")
            .await
            .unwrap()
            .is_some());

        // 2. TEST : Suppression par ID (Alice)
        manager
            .delete_identity("users", EntityIdentity::Id("u_100".to_string()))
            .await
            .expect("La suppression par ID devrait réussir");

        let fetch_alice = manager.get_document("users", "u_100").await.unwrap();
        assert!(
            fetch_alice.is_none(),
            "Alice (u_100) devrait être supprimée de la base"
        );

        // 3. TEST : Suppression par Nom (Bob)
        manager
            .delete_identity("users", EntityIdentity::Name("Bob".to_string()))
            .await
            .expect("La suppression par Nom devrait réussir");

        let fetch_bob = manager.get_document("users", "u_200").await.unwrap();
        assert!(
            fetch_bob.is_none(),
            "Bob (u_200) devrait être supprimé après résolution du nom"
        );

        // 4. TEST : Gérer l'erreur si le document n'existe pas
        let res = manager
            .delete_identity("users", EntityIdentity::Name("Fantome".to_string()))
            .await;

        assert!(
            res.is_err(),
            "La tentative de suppression d'un nom inexistant doit échouer"
        );
        let err_str = res.unwrap_err().to_string();
        assert!(err_str.contains("ERR_DB_ENTITY_NOT_FOUND"));
    }

    // 🎯 NOUVEAU TEST 1 : La Tolérance Zéro (Fail-Fast)
    #[tokio::test]
    async fn test_manager_fail_fast_on_missing_index() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_fail", "db_fail");

        // 1. Initialisation normale
        manager.init_db().await.unwrap();

        // 2. 🚨 SIMULATION DE CORRUPTION : On supprime physiquement l'index de la base
        let sys_path = manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("_system.json");
        crate::utils::io::remove_file(&sys_path).await.unwrap();

        // 3. On tente une insertion : le système DOIT bloquer immédiatement
        let doc = json!({ "id": "1", "name": "Test Fail Fast" });
        let res = manager.insert_raw("users", &doc).await;

        assert!(
            res.is_err(),
            "L'insertion aurait dû échouer car _system.json a été supprimé !"
        );
        let err_msg = res.unwrap_err().to_string();
        assert!(
            err_msg.contains("ERR_DB_SYSTEM_INDEX_NOT_FOUND"),
            "L'erreur remontée n'est pas la bonne : {}",
            err_msg
        );
    }

    // 🎯 NOUVEAU TEST 2 : Nettoyage de l'index lors de la suppression d'un item
    #[tokio::test]
    async fn test_manager_remove_item_from_index() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");

        manager.init_db().await.unwrap();
        manager.create_collection("users", None).await.unwrap();

        // 1. On insère un document (ce qui appelle add_item_to_index)
        let doc = json!({ "id": "u1", "name": "Alice" });
        manager.insert_raw("users", &doc).await.unwrap();

        // Vérification que le document est bien inscrit dans _system.json
        let index = manager.load_index().await.unwrap();
        let items = index["collections"]["users"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "Le document devrait être dans l'index");
        assert_eq!(items[0]["file"], "u1.json");

        // 2. On supprime le document (ce qui appelle remove_item_from_index)
        manager.delete_document("users", "u1").await.unwrap();

        // 3. Vérification que l'index a bien été purgé
        let index_after = manager.load_index().await.unwrap();
        let items_after = index_after["collections"]["users"]["items"]
            .as_array()
            .unwrap();
        assert!(
            items_after.is_empty(),
            "L'index devrait être vide après suppression"
        );
    }

    // 🎯 NOUVEAU TEST 3 : Nettoyage de l'index lors du drop d'une collection
    #[tokio::test]
    async fn test_manager_remove_collection_from_index() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");

        manager.init_db().await.unwrap();

        // 1. On crée une collection dynamique
        manager.create_collection("temporary", None).await.unwrap();

        let index = manager.load_index().await.unwrap();
        assert!(
            index["collections"].get("temporary").is_some(),
            "La collection devrait exister dans l'index"
        );

        // 2. On supprime la collection (ce qui appelle remove_collection_from_system_index)
        manager.drop_collection("temporary").await.unwrap();

        // 3. On vérifie la disparition totale dans l'index
        let index_after = manager.load_index().await.unwrap();
        assert!(
            index_after["collections"].get("temporary").is_none(),
            "La collection devrait avoir disparu de l'index"
        );
    }
}
