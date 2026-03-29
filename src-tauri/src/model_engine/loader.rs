// FICHIER : src-tauri/src/model_engine/loader.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::JsonLdProcessor;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::types::{ArcadiaElement, NameType, ProjectMeta, ProjectModel};
use crate::rules_engine::evaluator::DataProvider;
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*;
use tauri::State;

/// Index de localisation : Document_ID -> (Couche_DB, Nom_Collection)
type LocationIndex = UnorderedMap<String, (String, String)>;

pub struct ModelLoader<'a> {
    pub manager: CollectionsManager<'a>,
    /// Index partagé protégé par un verrou asynchrone
    index: SharedRef<AsyncRwLock<LocationIndex>>,
    processor: JsonLdProcessor,
}

impl<'a> ModelLoader<'a> {
    pub fn new(storage: &'a State<'_, StorageEngine>, space: &str, db: &str) -> Self {
        Self::from_engine(storage.inner(), space, db)
    }

    pub fn from_engine(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            manager: CollectionsManager::new(storage, space, db),
            index: SharedRef::new(AsyncRwLock::new(UnorderedMap::new())),
            processor: JsonLdProcessor::new(),
        }
    }

    pub fn new_with_manager(manager: CollectionsManager<'a>) -> Self {
        Self {
            manager,
            index: SharedRef::new(AsyncRwLock::new(UnorderedMap::new())),
            processor: JsonLdProcessor::new(),
        }
    }

    /// Analyse la structure du projet sur disque via le mapping ontologique
    pub async fn index_project(&self) -> RaiseResult<usize> {
        let mut idx = self.index.write().await;
        idx.clear();

        let config = AppConfig::get();
        let sys_mgr = CollectionsManager::new(
            self.manager.storage,
            &config.system_domain,
            &config.system_db,
        );

        // Lecture du mapping pour connaître les collections à scanner
        let mapping_doc = match sys_mgr
            .get_document("configs", "ref:configs:handle:ontological_mapping")
            .await?
        {
            Some(doc) => doc,
            None => return Ok(0), // Si pas de mapping, index vide
        };

        // 🎯 FIX : Utilisation d'un match explicite pour éviter l'erreur de conversion ?
        let search_spaces = match mapping_doc["search_spaces"].as_array() {
            Some(arr) => arr,
            None => raise_error!(
                "ERR_INVALID_ONTOLOGY_MAPPING",
                error = "Le champ 'search_spaces' est manquant ou invalide."
            ),
        };

        let mut count = 0;
        for space_def in search_spaces {
            let layer_db = space_def["layer"].as_str().unwrap_or("raise");
            let col = space_def["collection"].as_str().unwrap_or("");

            // Scan du système de fichiers pour récupérer les IDs (fichiers .json)
            let ids = self.fetch_document_ids(layer_db, col).await?;
            for id in ids {
                idx.insert(id.clone(), (layer_db.to_string(), col.to_string()));
                count += 1;
            }
        }
        Ok(count)
    }

    /// Récupère la liste des IDs de documents présents dans une collection physique
    async fn fetch_document_ids(&self, db: &str, col: &str) -> RaiseResult<Vec<String>> {
        let col_path = self
            .manager
            .storage
            .config
            .db_collection_path(&self.manager.space, db, col);
        let mut ids = Vec::new();

        if fs::exists_async(&col_path).await {
            let mut entries = fs::read_dir_async(&col_path).await?;
            while let Some(entry) = entries.next_entry().await? {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                        if !stem.starts_with('_') {
                            ids.push(stem.to_string());
                        }
                    }
                }
            }
        }
        Ok(ids)
    }

    /// Charge un élément spécifique par son ID
    pub async fn get_element(&self, id: &str) -> RaiseResult<ArcadiaElement> {
        let location = {
            let idx = self.index.read().await;
            idx.get(id).cloned()
        };

        match location {
            Some((db, col)) => {
                let target_mgr =
                    CollectionsManager::new(self.manager.storage, &self.manager.space, &db);
                let doc_opt = target_mgr.get_document(&col, id).await?;

                // 🎯 FIX : Utilisation d'un match explicite au lieu de ok_or_else
                let doc = match doc_opt {
                    Some(d) => d,
                    None => raise_error!(
                        "ERR_DB_INDEX_OUT_OF_SYNC",
                        error = format!("Document '{}' introuvable dans {}/{}", id, db, col)
                    ),
                };

                self.json_to_element(doc, Some(&db))
            }
            None => raise_error!(
                "ERR_DB_UNKNOWN_IDENTITY",
                error = format!("ID '{}' non répertorié dans l'index du projet", id)
            ),
        }
    }

    /// Transforme un document JSON en ArcadiaElement Pure Graph
    fn json_to_element(
        &self,
        doc: JsonValue,
        layer_hint: Option<&str>,
    ) -> RaiseResult<ArcadiaElement> {
        let id = doc
            .get("_id")
            .or(doc.get("id"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let name_val = doc
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Sans nom");

        let raw_type = doc
            .get("type")
            .or(doc.get("@type"))
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown");

        // Résolution dynamique du type via le processeur JSON-LD
        let kind = if let Some(layer) = layer_hint {
            let mut local_proc = self.processor.clone();
            let _ = local_proc.load_layer_context(layer);
            local_proc.context_manager().expand_term(raw_type)
        } else {
            raw_type.to_string()
        };

        // 🎯 PURE GRAPH : On aplatit toutes les propriétés dans la map dynamique
        let mut properties = UnorderedMap::new();
        if let Some(obj) = doc.as_object() {
            for (k, v) in obj {
                if !matches!(
                    k.as_str(),
                    "id" | "_id" | "name" | "type" | "@type" | "@context"
                ) {
                    properties.insert(k.clone(), v.clone());
                }
            }
        }

        Ok(ArcadiaElement {
            id,
            name: NameType::String(name_val.to_string()),
            kind,
            properties,
        })
    }

    /// Charge l'intégralité du modèle en mémoire
    pub async fn load_full_model(&self) -> RaiseResult<ProjectModel> {
        let count = self.index_project().await?;
        let index_snapshot = { self.index.read().await.clone() };

        let mut model = ProjectModel {
            meta: ProjectMeta {
                name: format!("{}/{}", self.manager.space, self.manager.db),
                element_count: count,
            },
            ..Default::default()
        };

        for (id, (layer, col)) in index_snapshot {
            if let Ok(el) = self.get_element(&id).await {
                // 🎯 PURE GRAPH : Remplissage dynamique sans dispatch statique
                model.add_element(&layer, &col, el);
            }
        }

        Ok(model)
    }
}

/// Implémentation du pont pour le moteur de règles (Data-Driven)
#[async_interface]
impl<'a> DataProvider for ModelLoader<'a> {
    async fn get_value(&self, _collection: &str, id: &str, field: &str) -> Option<JsonValue> {
        let el = self.get_element(id).await.ok()?;
        // Recherche dans les propriétés dynamiques
        el.properties.get(field).cloned()
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_loader_json_to_element_pure_graph() {
        let sandbox = AgentDbSandbox::new().await;
        let loader = ModelLoader::from_engine(&sandbox.db, "space", "db");

        let doc = json_value!({
            "_id": "el_1",
            "name": "Moteur",
            "type": "Component",
            "description": "Un moteur puissant",
            "mass": 450
        });

        let element = loader.json_to_element(doc, None).unwrap();

        assert_eq!(element.id, "el_1");
        assert_eq!(element.name.as_str(), "Moteur");

        // Vérification du stockage dynamique (Pure Graph)
        assert_eq!(
            element
                .properties
                .get("description")
                .unwrap()
                .as_str()
                .unwrap(),
            "Un moteur puissant"
        );
        assert_eq!(
            element.properties.get("mass").unwrap().as_i64().unwrap(),
            450
        );
    }
}
