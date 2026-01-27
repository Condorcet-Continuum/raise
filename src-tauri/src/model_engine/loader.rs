// FICHIER : src-tauri/src/model_engine/loader.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::arcadia; // <-- Accès au vocabulaire sémantique
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectMeta, ProjectModel};
use crate::rules_engine::evaluator::DataProvider;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::Utc;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use tokio::sync::RwLock;

/// Cache de localisation pour le Lazy Loading.
/// Mappe un UUID vers le nom de sa collection (ex: "550e8400-..." -> "oa").
type LocationIndex = HashMap<String, String>;

pub struct ModelLoader<'a> {
    pub manager: CollectionsManager<'a>,
    /// Index en mémoire pour localiser un élément sans scanner le disque
    index: Arc<RwLock<LocationIndex>>,
}

impl<'a> ModelLoader<'a> {
    // --- CONSTRUCTEURS ---

    pub fn new(storage: &'a State<'_, StorageEngine>, space: &str, db: &str) -> Self {
        Self::from_engine(storage.inner(), space, db)
    }

    pub fn from_engine(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            manager: CollectionsManager::new(storage, space, db),
            index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn new_with_manager(manager: CollectionsManager<'a>) -> Self {
        Self {
            manager,
            index: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    // --- INDEXATION (INITIALISATION RAPIDE) ---

    pub async fn index_project(&self) -> Result<usize> {
        let mut idx = self.index.write().await;
        idx.clear();

        // Liste des collections standard Arcadia
        let collections = ["oa", "sa", "la", "pa", "epbs", "data", "common"];
        let mut count = 0;

        for col in collections {
            let col_path = self.manager.storage.config.db_collection_path(
                &self.manager.space,
                &self.manager.db,
                col,
            );

            if col_path.exists() {
                let mut entries = tokio::fs::read_dir(col_path).await?;
                while let Some(entry) = entries.next_entry().await? {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("json") {
                        if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                            if !stem.starts_with('_') {
                                idx.insert(stem.to_string(), col.to_string());
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    // --- ACCÈS UNITAIRE (LAZY LOADING) ---

    pub async fn get_element(&self, id: &str) -> Result<ArcadiaElement> {
        let collection = {
            let idx = self.index.read().await;
            idx.get(id).cloned()
        };

        match collection {
            Some(col) => {
                let doc = self.manager.get_document(&col, id).await?.ok_or_else(|| {
                    anyhow!("Document {} introuvable dans {} (Index périmé ?)", id, col)
                })?;

                self.json_to_element(doc)
            }
            None => Err(anyhow!("ID inconnu ou non indexé : {}", id)),
        }
    }

    pub async fn get_json(&self, id: &str) -> Result<Value> {
        let collection = {
            let idx = self.index.read().await;
            idx.get(id).cloned()
        };

        if let Some(col) = collection {
            self.manager
                .get_document(&col, id)
                .await?
                .ok_or_else(|| anyhow!("Document introuvable"))
        } else {
            Err(anyhow!("ID non trouvé dans l'index"))
        }
    }

    fn json_to_element(&self, doc: Value) -> Result<ArcadiaElement> {
        // Utilisation des constantes sémantiques pour parser le JSON
        let id = doc
            .get(arcadia::PROP_ID)
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let short_type = doc
            .get("@type")
            .or_else(|| doc.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or(arcadia::KIND_UNKNOWN);

        let kind = self.resolve_uri_from_shortname(short_type);

        let name_val = doc
            .get(arcadia::PROP_NAME)
            .and_then(|v| v.as_str())
            .unwrap_or("Sans nom");
        let name = NameType::String(name_val.to_string());

        let description = doc
            .get(arcadia::PROP_DESCRIPTION)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let obj = doc.as_object().ok_or(anyhow!("Document invalide"))?;
        let mut properties = HashMap::new();
        for (k, v) in obj {
            // On exclut les champs standards déjà traités
            if !matches!(
                k.as_str(),
                "id" | "name" | "description" | "@type" | "type" | "@context" | "$schema"
            ) {
                properties.insert(k.clone(), v.clone());
            }
        }

        Ok(ArcadiaElement {
            id,
            name,
            kind,
            description,
            properties,
        })
    }

    // --- HYDRATATION CIBLÉE ---

    pub async fn fetch_hydrated_element(&self, element_id: &str) -> Result<Value> {
        let mut element = self.get_json(element_id).await?;

        // Liste des relations standard à hydrater (Utilisation des constantes pour éviter les typos)
        let relations = [
            arcadia::PROP_OWNED_LOGICAL_COMPONENTS,
            arcadia::PROP_OWNED_SYSTEM_COMPONENTS,
            arcadia::PROP_ALLOCATED_FUNCTIONS,
            arcadia::PROP_INCOMING_EXCHANGES,
            arcadia::PROP_OUTGOING_EXCHANGES,
            "ownedFunctionalAllocation", // Conserver pour compatibilité si pas dans constantes
            "base_class",
            "deployedComponents",
        ];

        self.hydrate_element(&mut element, &relations).await?;
        Ok(element)
    }

    pub async fn hydrate_element(&self, element: &mut Value, fields: &[&str]) -> Result<()> {
        for field in fields {
            if let Some(target_val) = element.get_mut(*field) {
                if let Some(arr) = target_val.as_array_mut() {
                    let mut hydrated_list = Vec::new();
                    for item in arr.iter() {
                        let target_id = item
                            .as_str()
                            .or_else(|| item.get("target").and_then(|t| t.as_str()));

                        if let Some(tid) = target_id {
                            if let Ok(doc) = self.get_json(tid).await {
                                hydrated_list.push(doc);
                            } else {
                                hydrated_list.push(item.clone());
                            }
                        } else {
                            hydrated_list.push(item.clone());
                        }
                    }
                    *target_val = serde_json::json!(hydrated_list);
                } else if let Some(tid) = target_val.as_str() {
                    if let Ok(doc) = self.get_json(tid).await {
                        *target_val = doc;
                    }
                }
            }
        }
        Ok(())
    }

    // --- CHARGEMENT COMPLET (Legacy) ---

    pub async fn load_full_model(&self) -> Result<ProjectModel> {
        if self.index.read().await.is_empty() {
            self.index_project().await?;
        }

        let mut model = ProjectModel {
            meta: ProjectMeta {
                name: format!("{}/{}", self.manager.space, self.manager.db),
                loaded_at: Utc::now().to_rfc3339(),
                element_count: 0,
            },
            ..Default::default()
        };

        let all_ids: Vec<String> = self.index.read().await.keys().cloned().collect();
        model.meta.element_count = all_ids.len();

        for id in all_ids {
            if let Ok(el) = self.get_element(&id).await {
                self.dispatch_element(&mut model, el);
            }
        }

        Ok(model)
    }

    fn dispatch_element(&self, model: &mut ProjectModel, el: ArcadiaElement) {
        let k = &el.kind;

        // Dispatch basé sur l'URI complète ou des marqueurs
        // OA
        if k.contains("/oa") || k.contains("Operational") {
            if k == arcadia::KIND_OA_ACTOR || k.contains("Actor") {
                model.oa.actors.push(el);
            } else if k == arcadia::KIND_OA_ACTIVITY || k.contains("Activity") {
                model.oa.activities.push(el);
            } else if k == arcadia::KIND_OA_CAPABILITY || k.contains("Capability") {
                model.oa.capabilities.push(el);
            } else if k == arcadia::KIND_OA_EXCHANGE || k.contains("Exchange") {
                model.oa.exchanges.push(el);
            } else {
                model.oa.entities.push(el);
            }
        }
        // SA
        else if k.contains("/sa") || k.contains("System") {
            if k == arcadia::KIND_SA_ACTOR || k.contains("Actor") {
                model.sa.actors.push(el);
            } else if k == arcadia::KIND_SA_FUNCTION || k.contains("Function") {
                model.sa.functions.push(el);
            } else if k == arcadia::KIND_SA_COMPONENT || k.contains("Component") {
                model.sa.components.push(el);
            } else if k == arcadia::KIND_SA_EXCHANGE || k.contains("Exchange") {
                model.sa.exchanges.push(el);
            } else {
                model.sa.capabilities.push(el);
            }
        }
        // LA
        else if k.contains("/la") || k.contains("Logical") {
            if k == arcadia::KIND_LA_COMPONENT || k.contains("Component") {
                model.la.components.push(el);
            } else if k == arcadia::KIND_LA_FUNCTION || k.contains("Function") {
                model.la.functions.push(el);
            } else if k == arcadia::KIND_LA_ACTOR || k.contains("Actor") {
                model.la.actors.push(el);
            } else if k == arcadia::KIND_LA_INTERFACE || k.contains("Interface") {
                model.la.interfaces.push(el);
            } else {
                model.la.exchanges.push(el);
            }
        }
        // PA
        else if k.contains("/pa") || k.contains("Physical") {
            if k == arcadia::KIND_PA_COMPONENT || k.contains("Component") {
                model.pa.components.push(el);
            } else if k == arcadia::KIND_PA_FUNCTION || k.contains("Function") {
                model.pa.functions.push(el);
            } else if k == arcadia::KIND_PA_LINK || k.contains("Link") {
                model.pa.links.push(el);
            } else {
                model.pa.actors.push(el);
            }
        }
        // Data
        else if k == arcadia::KIND_DATA_CLASS || k.contains("Class") {
            model.data.classes.push(el);
        }
        // EPBS
        else {
            model.epbs.configuration_items.push(el);
        }
    }

    /// Résolution des noms courts vers les URIs Arcadia officielles.
    /// Utilise maintenant le vocabulaire centralisé.
    fn resolve_uri_from_shortname(&self, short: &str) -> String {
        match short {
            // OA
            "OperationalActor" => arcadia::KIND_OA_ACTOR.to_string(),
            "OperationalActivity" => arcadia::KIND_OA_ACTIVITY.to_string(),
            "OperationalCapability" => arcadia::KIND_OA_CAPABILITY.to_string(),
            "OperationalExchange" => arcadia::KIND_OA_EXCHANGE.to_string(),
            "OperationalEntity" => arcadia::KIND_OA_ENTITY.to_string(),
            // SA
            "SystemFunction" => arcadia::KIND_SA_FUNCTION.to_string(),
            "SystemComponent" => arcadia::KIND_SA_COMPONENT.to_string(),
            "SystemActor" => arcadia::KIND_SA_ACTOR.to_string(),
            "SystemFunctionalExchange" => arcadia::KIND_SA_EXCHANGE.to_string(),
            // LA
            "LogicalFunction" => arcadia::KIND_LA_FUNCTION.to_string(),
            "LogicalComponent" => arcadia::KIND_LA_COMPONENT.to_string(),
            "LogicalActor" => arcadia::KIND_LA_ACTOR.to_string(),
            "LogicalInterface" => arcadia::KIND_LA_INTERFACE.to_string(),
            // PA
            "PhysicalFunction" => arcadia::KIND_PA_FUNCTION.to_string(),
            "PhysicalComponent" => arcadia::KIND_PA_COMPONENT.to_string(),
            "PhysicalLink" => arcadia::KIND_PA_LINK.to_string(),
            // Data
            "Class" => arcadia::KIND_DATA_CLASS.to_string(),
            // Fallback
            other => other.into(),
        }
    }
}

// --- IMPLEMENTATION RULES_ENGINE ---
#[async_trait]
impl<'a> DataProvider for ModelLoader<'a> {
    async fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<Value> {
        let doc_opt = if !collection.is_empty() {
            self.manager
                .get_document(collection, id)
                .await
                .ok()
                .flatten()
        } else {
            self.get_json(id).await.ok()
        };

        if let Some(doc) = doc_opt {
            let ptr = if field.starts_with('/') {
                field.to_string()
            } else {
                format!("/{}", field.replace('.', "/"))
            };
            doc.pointer(&ptr).cloned()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::rules_engine::evaluator::DataProvider;
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_loader_index_and_lazy_fetch() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_lazy", "db_lazy");
        manager.init_db().await.unwrap();

        // On utilise les constantes même dans les tests pour garantir la cohérence
        let doc = json!({
            "id": "UUID-LAZY-1",
            "name": "LazyComponent",
            "@type": "LogicalComponent"
        });
        manager.insert_raw("la", &doc).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);

        let count = loader.index_project().await.unwrap();
        assert_eq!(count, 1);

        let el = loader.get_element("UUID-LAZY-1").await.unwrap();
        assert_eq!(el.name.as_str(), "LazyComponent");
        // Vérification que l'URI est bien résolue via la constante
        assert_eq!(el.kind, arcadia::KIND_LA_COMPONENT);
    }

    #[tokio::test]
    async fn test_loader_hydration() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_hydro", "db_hydro");
        manager.init_db().await.unwrap();

        let parent = json!({
            "id": "PARENT-1",
            "name": "Motherboard",
            "ownedLogicalComponents": ["CHILD-1"]
        });
        let child = json!({
            "id": "CHILD-1",
            "name": "CPU"
        });

        manager.insert_raw("la", &parent).await.unwrap();
        manager.insert_raw("la", &child).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        loader.index_project().await.unwrap();

        let hydrated = loader.fetch_hydrated_element("PARENT-1").await.unwrap();
        let children = hydrated["ownedLogicalComponents"].as_array().unwrap();

        assert_eq!(children[0]["name"], "CPU");
    }

    #[tokio::test]
    async fn test_loader_as_data_provider() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_dp", "db_dp");
        manager.init_db().await.unwrap();

        let doc = json!({
            "id": "DOC-1",
            "status": { "active": true, "level": 5 }
        });
        manager.insert_raw("common", &doc).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        loader.index_project().await.unwrap();

        let val = loader.get_value("common", "DOC-1", "status.level").await;
        assert_eq!(val, Some(json!(5)));

        let val_idx = loader.get_value("", "DOC-1", "status.active").await;
        assert_eq!(val_idx, Some(json!(true)));
    }
}
