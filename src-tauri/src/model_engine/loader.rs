// FICHIER : src-tauri/src/model_engine/loader.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::vocabulary::{arcadia_types, namespaces};
use crate::json_db::jsonld::JsonLdProcessor;
use crate::json_db::storage::StorageEngine;
use anyhow::Result;
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;
use tauri::State;

// Import des types unifiés
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectMeta, ProjectModel};

pub struct ModelLoader<'a> {
    manager: CollectionsManager<'a>,
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
            processor: JsonLdProcessor::new(),
        }
    }

    pub fn new_with_manager(manager: CollectionsManager<'a>) -> Self {
        Self {
            manager,
            processor: JsonLdProcessor::new(),
        }
    }

    // --- LOGIQUE D'HYDRATATION (Pour la génération de code) ---

    pub async fn fetch_hydrated_element(&self, element_id: &str) -> Result<Value> {
        // 1. Récupération de l'objet brut (Migration async)
        let mut element = self
            .find_raw_element(element_id)
            .await
            .ok_or_else(|| anyhow::anyhow!("Élément introuvable : {}", element_id))?;

        // 2. Hydratation des relations clés
        let relations_to_hydrate = [
            "ownedFunctionalAllocation",
            "ownedLogicalComponents",
            "ownedSystemComponents",
            "base_class",
            "incomingFunctionalExchanges",
            "outgoingFunctionalExchanges",
        ];

        for rel in relations_to_hydrate {
            self.hydrate_relation(&mut element, rel).await?;
        }

        Ok(element)
    }

    async fn find_raw_element(&self, id: &str) -> Option<Value> {
        // Recherche dans toutes les collections probables (Migration async)
        let collections = ["la", "pa", "sa", "oa", "epbs", "data", "common"];
        for col_name in collections {
            if let Ok(Some(doc)) = self.manager.get_document(col_name, id).await {
                return Some(doc);
            }
        }
        None
    }

    async fn hydrate_relation(&self, element: &mut Value, field: &str) -> Result<()> {
        let relations_opt = element.get(field).cloned();

        if let Some(relations) = relations_opt {
            if let Some(arr) = relations.as_array() {
                let mut hydrated_list = Vec::new();
                for item in arr {
                    // Supporte soit un string ID, soit un objet {"target": "ID"}
                    let target_id = if let Some(s) = item.as_str() {
                        Some(s)
                    } else {
                        item.get("target").and_then(|v| v.as_str())
                    };

                    if let Some(tid) = target_id {
                        if let Some(obj) = self.find_raw_element(tid).await {
                            hydrated_list.push(obj);
                        } else {
                            hydrated_list.push(item.clone());
                        }
                    }
                }
                if !hydrated_list.is_empty() {
                    element[field] = json!(hydrated_list);
                }
            } else if let Some(s) = relations.as_str() {
                if let Some(obj) = self.find_raw_element(s).await {
                    element[field] = obj;
                }
            }
        }
        Ok(())
    }

    // --- CHARGEMENT DU MODÈLE COMPLET ---

    pub async fn load_full_model(&self) -> Result<ProjectModel> {
        let mut model = ProjectModel {
            meta: ProjectMeta {
                name: format!("{}/{}", self.manager.space, self.manager.db),
                loaded_at: Utc::now().to_rfc3339(),
                element_count: 0,
            },
            ..Default::default()
        };

        // Migration async des listes
        if let Ok(collections) = self.manager.list_collections().await {
            for col_name in collections {
                if col_name.starts_with('_') {
                    continue;
                }

                if let Ok(docs) = self.manager.list_all(&col_name).await {
                    for doc in docs {
                        if let Ok(element) = self.process_document_semantically(doc) {
                            self.dispatch_element(&mut model, element);
                            model.meta.element_count += 1;
                        }
                    }
                }
            }
        }

        Ok(model)
    }

    fn process_document_semantically(&self, doc: Value) -> Result<ArcadiaElement> {
        let expanded = self.processor.expand(&doc);
        let id = self
            .processor
            .get_id(&expanded)
            .unwrap_or_else(|| "unknown".to_string());
        let type_uri = self.processor.get_type(&expanded).unwrap_or_default();
        let compacted = self.processor.compact(&doc);

        let name_val = compacted
            .get("name")
            .or_else(|| compacted.get("http://www.w3.org/2004/02/skos/core#prefLabel"))
            .and_then(|v| v.as_str())
            .unwrap_or("Sans nom");

        let name = NameType::String(name_val.to_string());

        let description = compacted
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let obj = compacted
            .as_object()
            .ok_or(anyhow::anyhow!("Not an object"))?;
        let mut properties = HashMap::new();
        for (k, v) in obj {
            if !k.starts_with('@') && k != "description" && k != "name" {
                properties.insert(k.clone(), v.clone());
            }
        }

        Ok(ArcadiaElement {
            id,
            name,
            kind: type_uri,
            description,
            properties,
        })
    }

    fn dispatch_element(&self, model: &mut ProjectModel, el: ArcadiaElement) {
        let kind = &el.kind;

        if kind == &arcadia_types::uri(namespaces::OA, arcadia_types::OA_ACTOR) {
            model.oa.actors.push(el);
        } else if kind == &arcadia_types::uri(namespaces::OA, arcadia_types::OA_ACTIVITY) {
            model.oa.activities.push(el);
        } else if kind == &arcadia_types::uri(namespaces::SA, arcadia_types::SA_FUNCTION) {
            model.sa.functions.push(el);
        } else if kind == &arcadia_types::uri(namespaces::SA, arcadia_types::SA_COMPONENT) {
            model.sa.components.push(el);
        } else if kind == &arcadia_types::uri(namespaces::LA, arcadia_types::LA_COMPONENT) {
            model.la.components.push(el);
        } else if kind == &arcadia_types::uri(namespaces::PA, arcadia_types::PA_COMPONENT) {
            model.pa.components.push(el);
        } else if kind == &arcadia_types::uri(namespaces::DATA, arcadia_types::DATA_CLASS) {
            model.data.classes.push(el);
        } else if kind.contains("Actor") {
            model.oa.actors.push(el);
        } else if kind.contains("Function") {
            model.sa.functions.push(el);
        } else if kind.contains("Component") {
            model.la.components.push(el);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use tempfile::tempdir;

    #[tokio::test] // Migration async
    async fn test_fetch_hydrated_element() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        manager.init_db().await.unwrap();

        // Création données (Migration async)
        let comp = json!({
            "id": "COMP_A",
            "name": "Main",
            "ownedFunctionalAllocation": ["FUNC_1"]
        });
        let func = json!({ "id": "FUNC_1", "name": "Compute" });

        manager.insert_raw("la", &comp).await.unwrap();
        manager.insert_raw("sa", &func).await.unwrap();

        // Test
        let loader = ModelLoader::new_with_manager(manager);
        let hydrated = loader.fetch_hydrated_element("COMP_A").await.unwrap();

        let allocs = hydrated["ownedFunctionalAllocation"].as_array().unwrap();
        assert_eq!(allocs[0]["name"], "Compute");
    }
}
