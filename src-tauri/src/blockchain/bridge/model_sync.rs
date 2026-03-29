// src-tauri/src/blockchain/bridge/model_sync.rs

use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::utils::prelude::*;
use crate::AppState;

/// Synchroniseur responsable de la mise à jour du modèle symbolique en mémoire.
pub struct ModelSync<'a> {
    app_state: &'a AppState,
}

impl<'a> ModelSync<'a> {
    pub fn new(app_state: &'a AppState) -> Self {
        Self { app_state }
    }

    /// Applique les mutations d'un commit au ProjectModel global.
    pub async fn sync_commit(&self, commit: &ArcadiaCommit) -> RaiseResult<()> {
        let mut model_guard = self.app_state.model.lock().await;

        for mutation in &commit.mutations {
            self.apply_mutation(&mut model_guard, mutation)?;
        }
        Ok(())
    }

    fn apply_mutation(&self, model: &mut ProjectModel, mutation: &Mutation) -> RaiseResult<()> {
        match mutation.operation {
            MutationOp::Create | MutationOp::Update => {
                let element: ArcadiaElement =
                    match json::deserialize_from_value(mutation.payload.clone()) {
                        Ok(el) => el,
                        Err(e) => raise_error!(
                            "ERR_SYNC_PAYLOAD_INVALID",
                            error = e,
                            context = json_value!({
                                "element_id": mutation.element_id,
                                "action": "deserialize_mutation_payload"
                            })
                        ),
                    };

                self.upsert_element(model, element)?;
            }
            MutationOp::Delete => {
                self.delete_element(model, &mutation.element_id)?;
            }
        }
        Ok(())
    }

    /// 🎯 PURE GRAPH : Insertion ou mise à jour dynamique
    fn upsert_element(&self, model: &mut ProjectModel, element: ArcadiaElement) -> RaiseResult<()> {
        // On détermine la destination à partir du type (kind) de l'élément
        let (layer, col) = self.map_kind_to_location(&element.kind);

        // Si l'élément existe déjà quelque part, on le met à jour
        let mut found = false;
        for collections in model.layers.values_mut() {
            for vec in collections.values_mut() {
                if let Some(pos) = vec.iter().position(|e| e.id == element.id) {
                    vec[pos] = element.clone();
                    found = true;
                    break;
                }
            }
            if found {
                break;
            }
        }

        // Sinon, on l'ajoute dans sa couche naturelle
        if !found {
            model.add_element(layer, col, element);
        }

        Ok(())
    }

    /// 🎯 PURE GRAPH : Recherche et suppression transversale dans toutes les couches
    fn delete_element(&self, model: &mut ProjectModel, id: &str) -> RaiseResult<()> {
        for collections in model.layers.values_mut() {
            for vec in collections.values_mut() {
                if let Some(pos) = vec.iter().position(|e| e.id == id) {
                    vec.remove(pos);
                    return Ok(());
                }
            }
        }

        crate::raise_error!(
            "ERR_SYNC_ELEMENT_NOT_FOUND",
            error = format!("Élément '{}' introuvable pour suppression.", id)
        );
    }

    /// Helper pour router les nouveaux éléments vers les couches par défaut
    fn map_kind_to_location(&self, kind: &str) -> (&'static str, &'static str) {
        if kind.contains("OperationalActor") {
            ("oa", "actors")
        } else if kind.contains("OperationalActivity") {
            ("oa", "activities")
        } else if kind.contains("SystemComponent") {
            ("sa", "components")
        } else if kind.contains("SystemFunction") {
            ("sa", "functions")
        } else if kind.contains("LogicalComponent") {
            ("la", "components")
        } else if kind.contains("PhysicalComponent") {
            ("pa", "components")
        } else if kind.contains("Requirement") {
            ("transverse", "requirements")
        } else {
            ("others", "elements")
        }
    }

    pub fn is_ready(&self) -> bool {
        true
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_state() -> AppState {
        AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        }
    }

    #[async_test]
    async fn test_upsert_new_element_pure_graph() {
        let state = create_test_state();
        let sync = ModelSync::new(&state);

        let mutation = Mutation {
            element_id: "urn:sa:comp1".into(),
            operation: MutationOp::Create,
            payload: json_value!({
                "id": "urn:sa:comp1",
                "type": "SystemComponent",
                "name": "Radar Unit"
            }),
        };

        sync.apply_mutation(&mut *state.model.lock().await, &mutation)
            .unwrap();

        let model = state.model.lock().await;
        let components = model.get_collection("sa", "components");
        assert_eq!(components.len(), 1);
        assert_eq!(components[0].name.as_str(), "Radar Unit");
    }

    #[async_test]
    async fn test_delete_element_pure_graph() {
        let state = create_test_state();
        let sync = ModelSync::new(&state);

        let mut model = state.model.lock().await;
        model.add_element(
            "la",
            "components",
            ArcadiaElement {
                id: "urn:la:ecu".into(),
                kind: "LogicalComponent".into(),
                ..Default::default()
            },
        );
        drop(model);

        let mutation = Mutation {
            element_id: "urn:la:ecu".into(),
            operation: MutationOp::Delete,
            payload: json_value!({}),
        };

        sync.apply_mutation(&mut *state.model.lock().await, &mutation)
            .unwrap();

        let model = state.model.lock().await;
        assert!(model.get_collection("la", "components").is_empty());
    }
}
