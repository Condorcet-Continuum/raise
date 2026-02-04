// FICHIER : src-tauri/src/model_engine/loader.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::JsonLdProcessor;
use crate::json_db::storage::StorageEngine;
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
type LocationIndex = HashMap<String, String>;

pub struct ModelLoader<'a> {
    pub manager: CollectionsManager<'a>,
    index: Arc<RwLock<LocationIndex>>,
    processor: JsonLdProcessor,
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
            processor: JsonLdProcessor::new(),
        }
    }

    pub fn new_with_manager(manager: CollectionsManager<'a>) -> Self {
        Self {
            manager,
            index: Arc::new(RwLock::new(HashMap::new())),
            processor: JsonLdProcessor::new(),
        }
    }

    // --- INDEXATION ---

    pub async fn index_project(&self) -> Result<usize> {
        let mut idx = self.index.write().await;
        idx.clear();

        let collections = [
            "oa",
            "sa",
            "la",
            "pa",
            "epbs",
            "data",
            "transverse",
            "common",
        ];
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

    // --- ACCÈS UNITAIRE ---

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
                self.json_to_element(doc, Some(&col))
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

    fn json_to_element(&self, doc: Value, layer_hint: Option<&str>) -> Result<ArcadiaElement> {
        let id = doc
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let name_val = doc
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Sans nom");
        let name = NameType::String(name_val.to_string());

        let description = doc
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let short_type = doc
            .get("@type")
            .or_else(|| doc.get("type"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        let kind = if let Some(layer) = layer_hint {
            let mut local_proc = self.processor.clone();
            let _ = local_proc.load_layer_context(layer);
            local_proc.context_manager().expand_term(short_type)
        } else {
            short_type.to_string()
        };

        let obj = doc.as_object().ok_or(anyhow!("Document invalide"))?;
        let mut properties = HashMap::new();
        for (k, v) in obj {
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

    // --- HYDRATATION ---

    pub async fn fetch_hydrated_element(&self, element_id: &str) -> Result<Value> {
        let mut element = self.get_json(element_id).await?;

        let relations = [
            "ownedLogicalComponents",
            "ownedSystemComponents",
            "allocatedFunctions",
            "incomingComponentExchanges",
            "outgoingComponentExchanges",
            "ownedFunctionalAllocation",
            "base_class",
            "deployedComponents",
            "realizedEntities",
            "realizedActivities",
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

    // --- CHARGEMENT COMPLET ---

    pub async fn load_full_model(&self) -> Result<ProjectModel> {
        let count = self.index_project().await?;

        let all_ids: Vec<String> = {
            let index = self.index.read().await;
            index.keys().cloned().collect()
        };

        let mut model = ProjectModel {
            meta: ProjectMeta {
                name: format!("{}/{}", self.manager.space, self.manager.db),
                loaded_at: Utc::now().to_rfc3339(),
                element_count: count,
                ..Default::default()
            },
            ..Default::default()
        };

        for id in all_ids {
            if let Ok(el) = self.get_element(&id).await {
                self.dispatch_element(&mut model, el);
            }
        }

        Ok(model)
    }

    fn dispatch_element(&self, model: &mut ProjectModel, el: ArcadiaElement) {
        let k = &el.kind;

        // Dispatch robuste basé sur l'URI ou le nom

        if k.contains("/oa#") || k.contains("Operational") {
            if k.contains("Actor") {
                model.oa.actors.push(el);
            } else if k.contains("Activity") {
                model.oa.activities.push(el);
            } else if k.contains("Capability") {
                model.oa.capabilities.push(el);
            } else if k.contains("Exchange") {
                model.oa.exchanges.push(el);
            } else {
                model.oa.entities.push(el);
            }
        } else if k.contains("/sa#") || k.contains("System") {
            if k.contains("Actor") {
                model.sa.actors.push(el);
            } else if k.contains("Function") {
                model.sa.functions.push(el);
            } else if k.contains("Component") {
                model.sa.components.push(el);
            } else if k.contains("Exchange") {
                model.sa.exchanges.push(el);
            } else {
                model.sa.capabilities.push(el);
            }
        } else if k.contains("/la#") || k.contains("Logical") {
            if k.contains("Component") {
                model.la.components.push(el);
            } else if k.contains("Function") {
                model.la.functions.push(el);
            } else if k.contains("Actor") {
                model.la.actors.push(el);
            } else if k.contains("Interface") {
                model.la.interfaces.push(el);
            } else {
                model.la.exchanges.push(el);
            }
        } else if k.contains("/pa#") || k.contains("Physical") {
            if k.contains("Component") {
                model.pa.components.push(el);
            } else if k.contains("Function") {
                model.pa.functions.push(el);
            } else if k.contains("Link") {
                model.pa.links.push(el);
            } else {
                model.pa.actors.push(el);
            }
        } else if k.contains("Class") {
            model.data.classes.push(el);
        } else if k.contains("DataType") {
            model.data.data_types.push(el);
        } else if k.contains("ExchangeItem") {
            model.data.exchange_items.push(el);
        }
        // CORRECTION : Ajout de "CommonDefinition" dans la condition principale
        else if k.contains("/transverse#")
            || k.contains("Requirement")
            || k.contains("Scenario")
            || k.contains("FunctionalChain")
            || k.contains("Constraint")
            || k.contains("CommonDefinition")
        {
            if k.contains("Requirement") {
                model.transverse.requirements.push(el);
            } else if k.contains("Scenario") {
                model.transverse.scenarios.push(el);
            } else if k.contains("FunctionalChain") {
                model.transverse.functional_chains.push(el);
            } else if k.contains("Constraint") {
                model.transverse.constraints.push(el);
            } else if k.contains("CommonDefinition") {
                model.transverse.common_definitions.push(el);
            } else {
                model.transverse.others.push(el);
            }
        } else {
            model.epbs.configuration_items.push(el);
        }
    }
}

// --- DATA PROVIDER ---

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
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_loader_index_and_semantic_resolution() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space1", "db1");
        manager.init_db().await.unwrap();

        let doc = json!({
            "id": "UUID-SEM-1", "name": "User", "@type": "OperationalActor"
        });
        manager.insert_raw("oa", &doc).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        loader.index_project().await.unwrap();
        let el = loader.get_element("UUID-SEM-1").await.unwrap();

        assert!(el.kind.contains("OperationalActor"));
        assert_eq!(el.name.as_str(), "User");
    }

    #[tokio::test]
    async fn test_transverse_dispatch_comprehensive() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_tr", "db_tr");
        manager.init_db().await.unwrap();

        manager
            .insert_raw(
                "transverse",
                &json!({ "id": "REQ-1", "name": "Req1", "type": "Requirement" }),
            )
            .await
            .unwrap();
        manager
            .insert_raw(
                "transverse",
                &json!({ "id": "SC-1", "name": "Scen1", "type": "Scenario" }),
            )
            .await
            .unwrap();
        manager
            .insert_raw(
                "transverse",
                &json!({ "id": "FC-1", "name": "Chain1", "type": "FunctionalChain" }),
            )
            .await
            .unwrap();
        manager
            .insert_raw(
                "transverse",
                &json!({ "id": "CST-1", "name": "Const1", "type": "Constraint" }),
            )
            .await
            .unwrap();
        manager
            .insert_raw(
                "transverse",
                &json!({ "id": "COM-1", "name": "Def1", "type": "CommonDefinition" }),
            )
            .await
            .unwrap();
        manager.insert_raw("transverse", &json!({ "id": "OTH-1", "name": "Other1", "type": "https://raise.io/ontology/arcadia/transverse#CustomThing" })).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        let model = loader.load_full_model().await.unwrap();

        assert_eq!(
            model.transverse.requirements.len(),
            1,
            "Requirement manquant"
        );
        assert_eq!(model.transverse.scenarios.len(), 1, "Scenario manquant");
        assert_eq!(
            model.transverse.functional_chains.len(),
            1,
            "FunctionalChain manquante"
        );
        assert_eq!(
            model.transverse.constraints.len(),
            1,
            "Constraint manquante"
        );
        assert_eq!(
            model.transverse.common_definitions.len(),
            1,
            "CommonDefinition manquante"
        );
        assert_eq!(model.transverse.others.len(), 1, "Other manquant");
    }

    #[tokio::test]
    async fn test_provider_access_on_transverse() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "space_prov", "db_prov");
        manager.init_db().await.unwrap();

        manager
            .insert_raw(
                "transverse",
                &json!({
                    "id": "REQ-TEST", "name": "Limit", "properties": { "max": 120 }
                }),
            )
            .await
            .unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        loader.index_project().await.unwrap();

        let name = loader.get_value("transverse", "REQ-TEST", "name").await;
        assert_eq!(name, Some(Value::String("Limit".to_string())));

        let max = loader
            .get_value("transverse", "REQ-TEST", "properties/max")
            .await;
        assert_eq!(max, Some(json!(120)));
    }
}
