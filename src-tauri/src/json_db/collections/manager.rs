// FICHIER : src-tauri/src/json_db/collections/manager.rs
use crate::utils::prelude::*;

use crate::json_db::indexes::IndexManager;
use crate::json_db::jsonld::{JsonLdProcessor, VocabularyRegistry};
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::schema::{SchemaRegistry, SchemaValidator};
use crate::json_db::storage::{file_storage, StorageEngine};
use crate::rules_engine::{Evaluator, Rule, RuleStore};

use super::collection;
use super::data_provider::CachedDataProvider;

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
                context = json_value!({ "required_schema": expected_uri })
            );
        }

        let existing_doc =
            file_storage::read_system_index(&self.storage.config, &self.space, &self.db).await?;
        let is_new = existing_doc.is_none();

        let mut system_doc = existing_doc.unwrap_or_else(|| {
            json_value!({
                "$schema": expected_uri,
                "handle": format!("{}_{}", self.space, self.db),
                "name": format!("{}_{}", self.space, self.db),
                "space": self.space,
                "database": self.db,
                "version": 1,
                "_p2p": { "revision": 1 },
                "collections": {},
                "rules": {},
                "schemas": { "v1": {} }
            })
        });
        // 🎯 2. On compile le validateur avec le registre AMORCÉ
        let validator = SchemaValidator::compile_with_registry(expected_uri, &reg)?;

        // 🎯 3. Cette fois, le validateur TROUVE le schéma et injecte les 'default'
        validator.compute_then_validate(&mut system_doc)?;

        // 🎯 4. On crée physiquement les dossiers basés sur le document hydraté
        let created =
            file_storage::create_db(&self.storage.config, &self.space, &self.db, &system_doc)
                .await?;

        if is_new || created {
            let schemas_dir = self
                .storage
                .config
                .db_schemas_root(&self.space, &self.db)
                .join("v1");
            if !schemas_dir.exists() {
                let _ = fs::ensure_dir_async(&schemas_dir).await;
            }

            // 🎯 5. On sauvegarde l'index avec toutes les collections système injectées
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

    pub async fn load_index(&self) -> RaiseResult<JsonValue> {
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
                context = json_value!({
                    "path": sys_path.to_string_lossy().to_string(), 
                    "db": self.db,
                    "space": self.space
                })
            );
        }

        fs::read_json_async(&sys_path).await
    }

    // ============================================================================
    // GESTION DES SCHÉMAS (DDL)
    // ============================================================================
    async fn get_registry_for_uri(&self, _uri: &str) -> RaiseResult<SchemaRegistry> {
        // Le registre charge désormais toute la hiérarchie par lui-même
        SchemaRegistry::from_db(&self.storage.config, &self.space, &self.db).await
    }

    /// Construit l'URI standardisée en utilisant le space et la db du manager
    pub fn build_schema_uri(&self, schema_name: &str) -> String {
        if schema_name.starts_with("db://")
            || schema_name.starts_with("http://")
            || schema_name.starts_with("https://")
        {
            return schema_name.to_string();
        }
        let relative_path = if let Some(idx) = schema_name.find("/schemas/v1/") {
            &schema_name[idx + "/schemas/v1/".len()..]
        } else {
            schema_name
                .trim_start_matches('/')
                .trim_start_matches("schemas/")
                .trim_start_matches("v1/")
                .trim_start_matches('/')
        };
        format!(
            "db://{}/{}/schemas/v1/{}",
            self.space, self.db, relative_path
        )
    }

    pub async fn create_schema_def(&self, schema_name: &str, schema: JsonValue) -> RaiseResult<()> {
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
        prop_def: JsonValue,
    ) -> RaiseResult<()> {
        let uri = self.build_schema_uri(schema_name);
        let mut reg = self.get_registry_for_uri(&uri).await?;
        reg.add_property(&uri, prop_name, prop_def).await
    }

    pub async fn alter_schema_property(
        &self,
        schema_name: &str,
        prop_name: &str,
        prop_def: JsonValue,
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

    pub async fn get_document(&self, collection: &str, id: &str) -> RaiseResult<Option<JsonValue>> {
        self.storage
            .read_document(&self.space, &self.db, collection, id)
            .await
    }

    pub async fn get(&self, collection: &str, id: &str) -> RaiseResult<Option<JsonValue>> {
        self.get_document(collection, id).await
    }

    pub async fn read_many(&self, collection: &str, ids: &[String]) -> RaiseResult<Vec<JsonValue>> {
        let mut docs = Vec::with_capacity(ids.len());
        for _id in ids {
            let doc_opt = match self.get_document(collection, _id).await {
                Ok(doc) => doc,
                Err(e) => raise_error!(
                    "ERR_DB_DOCUMENT_READ",
                    error = e,
                    context = json_value!({
                        "collection": collection,
                        "document_id": _id
                    })
                ),
            };
            let Some(doc) = doc_opt else {
                raise_error!(
                    "ERR_DB_CORRUPTION_INDEX_MISMATCH",
                    error = "Document indexé mais introuvable physiquement",
                    context = json_value!({ "_id": _id, "coll": collection })
                );
            };
            docs.push(doc);
        }
        Ok(docs)
    }

    pub async fn list_all(&self, collection: &str) -> RaiseResult<Vec<JsonValue>> {
        // ✅ CORRECTION : On passe self.storage au lieu de &self.storage.config
        // Cela permet à list_documents de taper dans le cache.
        collection::list_documents(self.storage, &self.space, &self.db, collection).await
    }

    pub async fn list_collections(&self) -> RaiseResult<Vec<String>> {
        // Reste inchangé car c'est une lecture de métadonnées de dossier
        collection::list_collection_names_fs(&self.storage.config, &self.space, &self.db).await
    }

    async fn save_system_index(&self, doc: &mut JsonValue) -> RaiseResult<()> {
        let expected_uri = "db://_system/_system/schemas/v1/db/index.schema.json".to_string();
        if let Some(obj) = doc.as_object_mut() {
            obj.insert(
                "$schema".to_string(),
                JsonValue::String(expected_uri.clone()),
            );
        }

        let reg = self.get_registry_for_uri(&expected_uri).await?;
        if reg.get_by_uri(&expected_uri).is_none() {
            raise_error!(
                "ERR_DB_SECURITY_VIOLATION",
                error = "Schéma d'index système introuvable ou non autorisé.",
                context = json_value!({
                    "required_uri": expected_uri,
                    "action": "enforce_system_integrity",
                    "hint": "Le schéma 'index.schema.json' doit impérativement résider dans '_system/_system/schemas/v1/db'."
                })
            );
        }
        // ✅ 1. Le Rules Engine a la priorité
        if let Err(e) =
            apply_business_rules(self, "_system_index", doc, None, &reg, &expected_uri).await
        {
            user_warn!(
                "WRN_SYSTEM_RULE_INDEX_FAIL", // 🎯 Identifiant i18n et Event ID unique
                json_value!({
                    "component": "INDEX_ENGINE",
                    "technical_error": e.to_string(),
                    "is_blocking": false,
                    "hint": "Vérifiez l'intégrité du schéma JSON-LD dans _system"
                })
            );
        }

        // ✅ 2. Le Validator (avec x_compute) calcule ce qui manque (_id, dates)
        if let Ok(validator) = SchemaValidator::compile_with_registry(&expected_uri, &reg) {
            if let Err(e) = validator.compute_then_validate(doc) {
                user_warn!(
                    "WRN_SYSTEM_INDEX_INVALID_RECOVER", // 🎯 Identifiant i18n et Event ID unique
                    json_value!({
                        "component": "INDEX_ENGINE",
                        "action": "FORCE_SAVE",
                        "technical_error": e.to_string(),
                        "hint": "L'index a été corrompu mais la Forteresse a forcé une récupération."
                    })
                );
            }
        }

        file_storage::write_system_index(&self.storage.config, &self.space, &self.db, doc).await?;
        Ok(())
    }

    // --- GESTION DES COLLECTIONS ---
    pub async fn create_collection(&self, name: &str, schema_uri: &str) -> RaiseResult<()> {
        if !self.storage.config.db_root(&self.space, &self.db).exists() {
            self.init_db().await?;
        }
        let final_schema_uri = self.build_schema_uri(schema_uri);
        let col_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, name);
        if !col_path.exists() {
            fs::ensure_dir_async(&col_path).await?;
        }

        let meta = json_value!({ "schema": final_schema_uri, "indexes": [] });
        let meta_path = col_path.join("_meta.json");

        fs::write_json_atomic_async(&meta_path, &meta).await?;

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
                context = json_value!({ "found_schema": current_schema })
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
                context = json_value!({
                    "target_name": col_name,
                    "searched_paths": [col_ptr, rule_ptr]
                })
            );
        };

        if path.is_empty() {
            return Ok(String::new());
        }

        // 3. Résolution de l'URI finale (db://...)
        Ok(self.build_schema_uri(path))
    }

    async fn update_system_index_collection(
        &self,
        col_name: &str,
        schema_uri: &str,
    ) -> RaiseResult<()> {
        let mut system_doc = self.load_index().await?; // 🎯 Douane stricte

        if system_doc.get("collections").is_none() {
            system_doc["collections"] = json_value!({});
        }
        if let Some(cols) = system_doc["collections"].as_object_mut() {
            let existing_items = cols
                .get(col_name)
                .and_then(|c| c.get("items"))
                .cloned()
                .unwrap_or(json_value!([]));
            cols.insert(
                col_name.to_string(),
                json_value!({ "schema": schema_uri, "items": existing_items }),
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
            system_doc["collections"] = json_value!({});
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
                    json_value!({ "schema": schema_guess, "items": [] }),
                );
            }
            if let Some(col_entry) = cols.get_mut(col_name) {
                if col_entry.get("items").is_none() {
                    col_entry["items"] = json_value!([]);
                }
                if let Some(items) = col_entry["items"].as_array_mut() {
                    if !items
                        .iter()
                        .any(|i| i.get("file").and_then(|f| f.as_str()) == Some(&filename))
                    {
                        items.push(json_value!({ "file": filename }));
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
    pub async fn insert_raw(&self, collection: &str, doc: &JsonValue) -> RaiseResult<()> {
        let Some(_id) = doc.get("_id").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_DB_DOCUMENT_ID_MISSING",
                error = "Attribut 'id' manquant ou format invalide dans le document",
                context = json_value!({
                    "expected_field": "_id",
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
            let schema_hint = doc.get("$schema").and_then(|s| s.as_str());
            if let Some(uri) = schema_hint {
                self.create_collection(collection, uri).await?;
            } else {
                raise_error!(
                    "ERR_DB_STRICT_SCHEMA_REQUIRED",
                    error = "Impossible de créer la collection à la volée : aucun '$schema' défini dans le document.",
                    context = json_value!({ "collection": collection })
                );
            }
        }

        self.storage
            .write_document(&self.space, &self.db, collection, _id, doc)
            .await?;
        self.add_item_to_index(collection, _id).await?;

        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        if let Err(_e) = idx_mgr.index_document(collection, doc).await {
            #[cfg(debug_assertions)]
            user_warn!(
                "WRN_SECONDARY_INDEX_FAILED", // 🎯 Identifiant i18n et Event ID unique
                json_value!({
                    "component": "INDEX_ENGINE",
                    "index_type": "secondary",
                    "technical_error": _e.to_string(),
                    "is_critical": false,
                    "hint": "La recherche sur cet index peut être dégradée, mais l'intégrité de la donnée source est préservée."
                })
            );
        }
        Ok(())
    }

    #[async_recursive]
    pub async fn insert_with_schema(
        &self,
        collection: &str,
        mut doc: JsonValue,
    ) -> RaiseResult<JsonValue> {
        doc = self.resolve_document_references(doc).await?;
        self.prepare_document(collection, &mut doc).await?;
        self.insert_raw(collection, &doc).await?;
        Ok(doc)
    }

    pub async fn update_document(
        &self,
        collection: &str,
        id: &str,
        patch_data: JsonValue,
    ) -> RaiseResult<JsonValue> {
        let resolved_patch = self.resolve_document_references(patch_data).await?;
        let old_doc_opt = self.get_document(collection, id).await?;
        let Some(mut doc) = old_doc_opt else {
            raise_error!(
                "ERR_DB_UPDATE_TARGET_NOT_FOUND",
                error = "Échec de la mise à jour : le document original est introuvable.",
                context = json_value!({ "action": "update_document" })
            );
        };
        json_merge(&mut doc, resolved_patch);

        if let Some(obj) = doc.as_object_mut() {
            // 1. SÉCURITÉ : On verrouille la clé primaire pour empêcher la mutation de l'ID
            obj.insert("_id".to_string(), JsonValue::String(id.to_string()));
            // 2. LOGIQUE D'ÉTAT P2P : On incrémente la révision (seul le code DB connaît l'ancien état)
            if let Some(p2p) = obj.get_mut("_p2p").and_then(|v| v.as_object_mut()) {
                if let Some(rev) = p2p.get("revision").and_then(|v| v.as_i64()) {
                    p2p.insert("revision".to_string(), json_value!(rev + 1));
                }
            }
        }

        self.prepare_document(collection, &mut doc).await?;

        self.storage
            .write_document(&self.space, &self.db, collection, id, &doc)
            .await?;

        let mut idx_mgr = IndexManager::new(self.storage, &self.space, &self.db);
        let _ = idx_mgr.index_document(collection, &doc).await;

        Ok(doc)
    }

    #[async_recursive]
    pub async fn upsert_document(
        &self,
        collection: &str,
        mut data: JsonValue,
    ) -> RaiseResult<String> {
        data = self.resolve_document_references(data).await?;

        // 🎯 1. EXTRACTION DE TOUTES LES CLÉS D'IDENTITÉ POSSIBLES
        let id_opt = data
            .get("_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let handle_opt = data.get("handle").cloned();
        let name_opt = data.get("name").cloned();

        if id_opt.is_none() && handle_opt.is_none() && name_opt.is_none() {
            raise_error!(
                "ERR_DB_UPSERT_MISSING_IDENTITY",
                error = "Identifiant manquant : l'upsert requiert '_id', 'handle' ou 'name'.",
                context = json_value!({
                    "action": "upsert_document",
                    "hint": "Fournissez une clé d'identification unique à la racine du document."
                })
            );
        }

        let mut target_id = None;

        // 🎯 2. RECHERCHE DIRECTE PAR ID (O(1))
        if let Some(ref id) = id_opt {
            if let Ok(Some(_)) = self.get_document(collection, id).await {
                target_id = Some(id.clone());
            }
        }

        if target_id.is_none() {
            // FIX : On priorise 'handle', puis 'name'
            let search_param = if let Some(v) = handle_opt {
                Some(("handle", v))
            } else {
                name_opt.map(|v| ("name", v))
            };

            // Si un champ alternatif existe, on interroge la base
            if let Some((field, value)) = search_param {
                let mut query = Query::new(collection);
                query.filter = Some(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![Condition::eq(field, value)],
                });
                query.limit = Some(1);

                if let Ok(result) = QueryEngine::new(self).execute_query(query).await {
                    if let Some(existing_doc) = result.documents.first() {
                        if let Some(found_id) = existing_doc.get("_id").and_then(|v| v.as_str()) {
                            target_id = Some(found_id.to_string());
                        }
                    }
                }
            }
        }

        // 🎯 4. EXÉCUTION DE L'ACTION DÉFINITIVE
        match target_id {
            Some(id) => {
                self.update_document(collection, &id, data).await?;
                Ok(format!("Updated: {}", id))
            }
            None => {
                let doc = self.insert_with_schema(collection, data).await?;
                let new_id = doc.get("_id").and_then(|v| v.as_str()).unwrap().to_string();
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

    #[async_recursive]
    pub async fn prepare_document(&self, collection: &str, doc: &mut JsonValue) -> RaiseResult<()> {
        // 1. DÉTERMINATION DU SCHÉMA
        let mut resolved_uri: Option<String> = None;

        let meta_path = self
            .storage
            .config
            .db_collection_path(&self.space, &self.db, collection)
            .join("_meta.json");

        if meta_path.exists() {
            if let Ok(content) = fs::read_to_string_async(&meta_path).await {
                if let Ok(meta) = json::deserialize_from_str::<JsonValue>(&content) {
                    if let Some(s) = meta.get("schema").and_then(|v| v.as_str()) {
                        if !s.is_empty() {
                            resolved_uri = Some(self.build_schema_uri(s));
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

        // 2. INJECTION, CALCULS & VALIDATION (SSOT via Validator)
        if let Some(uri) = &resolved_uri {
            if let Some(obj) = doc.as_object_mut() {
                obj.insert("$schema".to_string(), JsonValue::String(uri.clone()));
            }

            let reg = self.get_registry_for_uri(uri).await?;

            if let Err(e) = apply_business_rules(self, collection, doc, None, &reg, uri).await {
                user_warn!(
                    "WRN_BUSINESS_RULE_FAILURE", // 🎯 Identifiant i18n et Event ID unique
                    json_value!({
                        "component": "RULES_ENGINE",
                        "severity": "non_blocking",
                        "technical_error": e.to_string(),
                        "hint": "Une règle métier n'a pas pu être validée, mais l'opération continue."
                    })
                );
            }

            let validator = SchemaValidator::compile_with_registry(uri, &reg)?;
            validator.compute_then_validate(doc)?;

            // 3. 🛡️ CALCUL DU CHECKSUM P2P & ORIGIN_NODE
            if let Some(obj) = doc.as_object_mut() {
                let ws_id = AppConfig::get()
                    .workstation
                    .as_ref()
                    .map(|ws| ws.id.as_str())
                    .unwrap_or("unknown");

                let mut doc_for_hash = obj.clone();
                doc_for_hash.remove("_p2p");
                let hash = self.compute_document_checksum(&JsonValue::Object(doc_for_hash));

                if let Some(p2p) = obj.get_mut("_p2p").and_then(|v| v.as_object_mut()) {
                    p2p.insert("checksum".to_string(), json_value!(hash));
                    p2p.insert("origin_node".to_string(), json_value!(ws_id));
                    p2p.insert(
                        "last_sync_at".to_string(),
                        json_value!(UtcClock::now().to_rfc3339()),
                    );
                }
            }
        } else {
            raise_error!(
                "ERR_DB_STRICT_SCHEMA_REQUIRED",
                error = "Insertion refusée : Aucun schéma de validation n'est défini pour cette collection.",
                context = json_value!({ "collection": collection, "action": "prepare_document" })
            );
        }

        // 4. 🎯 LOGIQUE SÉMANTIQUE JSON-LD (AVEC raise_error!)
        if let Err(e) = self.apply_semantic_logic(doc, resolved_uri.as_deref()) {
            raise_error!(
                "ERR_AI_SEMANTIC_VALIDATION_FAIL",
                error = e.to_string(),
                context = json_value!({
                    "action": "semantic_integrity_check",
                    "collection": collection,
                    "resolved_uri": resolved_uri,
                    "hint": "Le document n'a pas pu être aligné avec l'ontologie JSON-LD."
                })
            );
        }

        Ok(())
    }

    // --- OPTIMISATION SEMANTIQUE ---
    fn apply_semantic_logic(
        &self,
        doc: &mut JsonValue,
        schema_uri: Option<&str>,
    ) -> RaiseResult<()> {
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
                        if let Ok(val) = json::serialize_to_value(defaults) {
                            obj.insert("@context".to_string(), val);
                        }
                    }
                } else {
                    let defaults = registry.get_default_context();
                    if let Ok(val) = json::serialize_to_value(defaults) {
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

            if let Some(type_uri) = processor.get_primary_type(doc) {
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
                    conditions: vec![Condition::eq("name", crate::utils::json::json_value!(name))],
                });
                let res = qe.execute_query(query).await?;

                match res.documents.first() {
                    Some(doc) => doc.get("_id").and_then(|v| v.as_str()).unwrap().to_string(),
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

    pub async fn resolve_document_references(&self, document: JsonValue) -> RaiseResult<JsonValue> {
        resolve_refs_recursive(document, self).await
    }

    fn compute_document_checksum(&self, doc: &JsonValue) -> String {
        use sha2::{Digest, Sha256};
        // Sérialisation du JSON en vecteur d'octets
        let content = serde_json::to_vec(doc).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(content);
        hex::encode(hasher.finalize())
    }
}

fn json_merge(a: &mut JsonValue, b: JsonValue) {
    match (a, b) {
        (JsonValue::Object(a), JsonValue::Object(b)) => {
            for (k, v) in b {
                json_merge(a.entry(k).or_insert(JsonValue::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}

#[allow(clippy::too_many_arguments)]
#[async_recursive]
pub async fn apply_business_rules(
    manager: &CollectionsManager<'_>,
    collection_name: &str,
    doc: &mut JsonValue,
    old_doc: Option<&JsonValue>,
    registry: &SchemaRegistry,
    schema_uri: &str,
) -> RaiseResult<()> {
    let mut store = RuleStore::new(manager);

    if let Err(e) = store.sync_from_db().await {
        user_warn!(
            "WRN_SYSTEM_RULES_LOAD_FAILED", // 🎯 Identifiant i18n et Event ID unique
            json_value!({
                "component": "RULES_ENGINE",
                "scope": "system_rules",
                "technical_error": e.to_string(),
                "hint": "Le moteur de règles fonctionnera avec les paramètres par défaut, mais certaines contraintes système pourraient être absentes."
            })
        );
    }

    if let Some(schema) = registry.get_by_uri(schema_uri) {
        if let Some(rules_array) = schema.get("x_rules").and_then(|v| v.as_array()) {
            for (index, rule_val) in rules_array.iter().enumerate() {
                match json::deserialize_from_value::<Rule>(rule_val.clone()) {
                    Ok(rule) => {
                        if let Err(e) = store.register_rule(collection_name, rule).await {
                            user_warn!(
                                "WRN_RULE_REGISTRATION_FAILED", // 🎯 Identifiant i18n et Event ID unique
                                json_value!({
                                    "component": "RULES_ENGINE",
                                    "item_index": index,
                                    "technical_error": e.to_string(),
                                    "hint": "Une règle spécifique n'a pas pu être enregistrée dans l'index. L'intégrité globale est maintenue."
                                })
                            );
                        }
                    }
                    Err(e) => {
                        user_warn!(
                            "WRN_SCHEMA_RULE_INVALID", // 🎯 Identifiant i18n et Event ID unique
                            json_value!({
                                "component": "RULES_ENGINE",
                                "item_index": index,
                                "technical_error": e.to_string(),
                                "hint": "Une règle du schéma JSON-LD est syntaxiquement incorrecte et a été ignorée."
                            })
                        );
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

        let mut next_changes = UniqueSet::new();
        for rule in rules {
            match Evaluator::evaluate(&rule.expr, doc, &provider).await {
                Ok(result) => {
                    if set_value_by_path(doc, &rule.target, result.into_owned()) {
                        next_changes.insert(rule.target.clone());
                    }
                }
                Err(AppError::Structured(ref data)) if data.code == "ERR_RULE_VAR_NOT_FOUND" => {
                    continue;
                }
                Err(e) => {
                    raise_error!(
                        "ERR_DB_RULE_EVAL_FAIL",
                        context = json_value!({
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

fn compute_diff(new_doc: &JsonValue, old_doc: Option<&JsonValue>) -> UniqueSet<String> {
    let mut changes = UniqueSet::new();
    find_changes("", new_doc, old_doc, &mut changes);
    changes
}

fn find_changes(
    path: &str,
    new_val: &JsonValue,
    old_val: Option<&JsonValue>,
    changes: &mut UniqueSet<String>,
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
        (JsonValue::Object(new_map), Some(JsonValue::Object(old_map))) => {
            for (k, v) in new_map {
                let new_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path, k)
                };
                find_changes(&new_path, v, old_map.get(k), changes);
            }
        }
        (JsonValue::Object(new_map), None) => {
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

fn set_value_by_path(doc: &mut JsonValue, path: &str, value: JsonValue) -> bool {
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
                *current = json_value!({});
            }
            if current.get(*part).is_none() {
                current
                    .as_object_mut()
                    .unwrap()
                    .insert(part.to_string(), json_value!({}));
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
    data: JsonValue,
    col_mgr: &'a CollectionsManager<'a>,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move {
        match data {
            JsonValue::String(s) => {
                if let Some((col, field, val)) = parse_smart_link(&s) {
                    let mut query = Query::new(col);
                    query.filter = Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: vec![Condition::eq(field, val.into())],
                    });

                    let result = QueryEngine::new(col_mgr).execute_query(query).await?;
                    if let Some(doc) = result.documents.first() {
                        let id = doc
                            .get("_id")
                            .or_else(|| doc.get("_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Ok(JsonValue::String(id.to_string()))
                    } else {
                        Ok(JsonValue::String(s))
                    }
                } else {
                    Ok(JsonValue::String(s))
                }
            }
            JsonValue::Array(arr) => {
                let mut new_arr = Vec::new();
                for item in arr {
                    new_arr.push(resolve_refs_recursive(item, col_mgr).await?);
                }
                Ok(JsonValue::Array(new_arr))
            }
            JsonValue::Object(map) => {
                let mut new_map = JsonObject::new();
                for (k, v) in map {
                    new_map.insert(k, resolve_refs_recursive(v, col_mgr).await?);
                }
                Ok(JsonValue::Object(new_map))
            }
            _ => Ok(data),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_manager_init_db_completeness() {
        // 1. Setup de l'environnement isolé
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "system_test", "db_test");

        // 2. Exécution de l'initialisation
        let created = manager.init_db().await.expect("L'initialisation a échoué");
        assert!(created, "La DB aurait dû être créée pour la première fois");

        // 3. Lecture directe de l'index système généré (_system.json)
        let index =
            file_storage::read_system_index(&sandbox.storage.config, "system_test", "db_test")
                .await
                .unwrap()
                .expect("Le fichier _system.json est introuvable");

        // --- ASSERTIONS SUR LE CONTENU DU JSON ---

        // Vérification de l'ID auto-généré (x_compute)
        assert!(
            index.get("_id").is_some(),
            "L'index devrait avoir un '_id' généré"
        );
        assert!(index["_id"].is_string());

        // Vérification de l'hydratation des collections par défaut
        // (Vérifie que le validator a bien injecté _migrations depuis le schéma)
        assert!(
            index["collections"].get("_migrations").is_some(),
            "La collection '_migrations' aurait dû être injectée par défaut"
        );

        assert_eq!(
            index["collections"]["_migrations"]["schema"],
            "db://_system/_system/schemas/v1/db/migration.schema.json"
        );

        // Vérification des règles système
        assert!(
            index["rules"].get("_system_rules").is_some(),
            "La règle '_system_rules' aurait dû être injectée par défaut"
        );

        // --- ASSERTIONS SUR LE SYSTÈME DE FICHIERS ---

        let db_root = sandbox.storage.config.db_root("system_test", "db_test");

        // Vérifie que le dossier physique de la collection _migrations existe
        let migration_path = db_root.join("collections/_migrations");
        assert!(
            migration_path.exists(),
            "Le dossier physique de _migrations est manquant"
        );

        // Vérifie que le dossier des schémas locaux a été créé
        let schema_path = db_root.join("schemas/v1");
        assert!(
            schema_path.exists(),
            "L'arborescence des schémas n'a pas été initialisée"
        );
    }

    #[async_test]
    async fn test_manager_get_document_integration() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
        manager.init_db().await.unwrap();
        manager
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // ✅ CORRECTION : Utilisation de _id
        let doc = json_value!({ "_id": "user_123", "name": "Test User" });
        manager.insert_raw("users", &doc).await.unwrap();

        let result = manager.get_document("users", "user_123").await.unwrap();
        assert!(result.is_some());
        assert_eq!(result.unwrap()["name"], "Test User");

        let missing = manager.get_document("users", "ghost").await.unwrap();
        assert!(missing.is_none());
    }

    #[async_test]
    async fn test_read_many_parallel() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
        manager.init_db().await.unwrap();
        manager
            .create_collection(
                "items",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        for i in 0..100 {
            // ✅ CORRECTION : Utilisation de _id
            let doc = json_value!({ "_id": i.to_string(), "val": i });
            manager.insert_raw("items", &doc).await.unwrap();
        }

        let ids: Vec<String> = vec!["10", "20", "50", "80", "99"]
            .into_iter()
            .map(String::from)
            .collect();
        let results = manager.read_many("items", &ids).await.unwrap();

        assert_eq!(results.len(), 5);
        for res in results {
            // ✅ CORRECTION : Vérification sur _id
            let id = res["_id"].as_str().unwrap();
            assert!(ids.contains(&id.to_string()));
        }
    }

    #[async_test]
    async fn test_read_many_strict_integrity() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
        manager.init_db().await.unwrap();
        manager
            .create_collection(
                "items",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // ✅ CORRECTION : Utilisation de _id
        manager
            .insert_raw("items", &json_value!({ "_id": "1", "val": "A" }))
            .await
            .unwrap();

        let ids = vec!["1".to_string(), "999".to_string()];
        let result = manager.read_many("items", &ids).await;

        assert!(result.is_err());
        let err_str = result.unwrap_err().to_string();
        assert!(err_str.contains("ERR_DB_CORRUPTION_INDEX_MISMATCH"));
    }

    #[async_test]
    async fn test_crud_workflow() {
        let sandbox = DbSandbox::new().await;
        let mgr = CollectionsManager::new(&sandbox.storage, "test", "crud");
        mgr.init_db().await.unwrap();

        mgr.create_collection(
            "items",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

        // 1. CREATE (Insert)
        // L'insertion via le schéma va auto-générer le _id grâce à validator.rs
        let doc = json_value!({ "name": "Item 1", "price": 100 });
        let created_doc = mgr.insert_with_schema("items", doc).await.unwrap();
        let id = created_doc["_id"].as_str().unwrap().to_string();

        let fetched = mgr.get_document("items", &id).await.unwrap();
        assert!(fetched.is_some());

        // 2. UPDATE
        mgr.update_document(
            "items",
            &id,
            json_value!({ "price": 150, "status": "active" }),
        )
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

    #[async_test]
    async fn test_upsert_idempotence() {
        let sandbox = DbSandbox::new().await;
        let mgr = CollectionsManager::new(&sandbox.storage, "test", "upsert");
        mgr.init_db().await.unwrap();

        mgr.create_collection(
            "configs",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

        // ✅ CORRECTION : Utilisation de _id
        let data1 = json_value!({ "_id": "config-01", "val": "A" });
        let res1 = mgr.upsert_document("configs", data1).await.unwrap();
        assert!(res1.contains("Created"));

        // ✅ CORRECTION : Utilisation de _id
        let data2 = json_value!({ "_id": "config-01", "val": "B" });
        let res2 = mgr.upsert_document("configs", data2).await.unwrap();
        assert!(res2.contains("Updated"));

        let final_doc = mgr
            .get_document("configs", "config-01")
            .await
            .unwrap()
            .unwrap();

        assert_eq!(final_doc["val"], "B");
        // ✅ CORRECTION : Vérification sur _id
        assert_eq!(final_doc["_id"], "config-01");
    }

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

    #[async_test]
    async fn test_manager_delete_identity() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");
        manager.init_db().await.unwrap();

        manager
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // ✅ CORRECTION : Remplacement du doublon "name" par "_id"
        let doc_alice = json_value!({ "_id": "u_100", "name": "Alice" });
        let doc_bob = json_value!({ "_id": "u_200", "name": "Bob" });

        manager.insert_raw("users", &doc_alice).await.unwrap();
        manager.insert_raw("users", &doc_bob).await.unwrap();

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

    #[async_test]
    async fn test_manager_fail_fast_on_missing_index() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_fail", "db_fail");

        manager.init_db().await.unwrap();
        manager
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let sys_path = manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("_system.json");
        fs::remove_file_async(&sys_path).await.unwrap();

        // ✅ CORRECTION : Utilisation de _id au lieu de handle
        let doc = json_value!({ "_id": "1", "name": "Test Fail Fast" });
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

    #[async_test]
    async fn test_manager_remove_item_from_index() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");

        manager.init_db().await.unwrap();
        manager
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // ✅ CORRECTION : Utilisation de _id au lieu de handle
        let doc = json_value!({ "_id": "u1", "name": "Alice" });
        manager.insert_raw("users", &doc).await.unwrap();

        let index = manager.load_index().await.unwrap();
        let items = index["collections"]["users"]["items"].as_array().unwrap();
        assert_eq!(items.len(), 1, "Le document devrait être dans l'index");
        assert_eq!(items[0]["file"], "u1.json");

        manager.delete_document("users", "u1").await.unwrap();

        let index_after = manager.load_index().await.unwrap();
        let items_after = index_after["collections"]["users"]["items"]
            .as_array()
            .unwrap();
        assert!(
            items_after.is_empty(),
            "L'index devrait être vide après suppression"
        );
    }

    #[async_test]
    async fn test_manager_remove_collection_from_index() {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(&sandbox.storage, "space_test", "db_test");

        manager.init_db().await.unwrap();

        manager
            .create_collection(
                "temporary",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let index = manager.load_index().await.unwrap();
        assert!(
            index["collections"].get("temporary").is_some(),
            "La collection devrait exister dans l'index"
        );

        manager.drop_collection("temporary").await.unwrap();

        let index_after = manager.load_index().await.unwrap();
        assert!(
            index_after["collections"].get("temporary").is_none(),
            "La collection devrait avoir disparu de l'index"
        );
    }
}
