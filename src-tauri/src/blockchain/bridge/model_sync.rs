// src-tauri/src/blockchain/bridge/model_sync.rs

use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::utils::{data, prelude::*};
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
    pub fn sync_commit(&self, commit: &ArcadiaCommit) -> Result<()> {
        let mut model_guard = self
            .app_state
            .model
            .lock()
            .map_err(|_| AppError::from("Mutex du ProjectModel empoisonné"))?;

        for mutation in &commit.mutations {
            // Utilisation de l'auto-deref pour la garde du Mutex (Validation Clippy)
            self.apply_mutation(&mut model_guard, mutation)?;
        }
        Ok(())
    }

    fn apply_mutation(&self, model: &mut ProjectModel, mutation: &Mutation) -> Result<()> {
        match mutation.operation {
            MutationOp::Create | MutationOp::Update => {
                let element: ArcadiaElement =
                    data::from_value(mutation.payload.clone()).map_err(|e| {
                        AppError::from(format!("Payload invalide pour ArcadiaElement: {}", e))
                    })?;

                self.upsert_element(model, element)?;
            }
            MutationOp::Delete => {
                self.delete_element(model, &mutation.element_id)?;
            }
        }
        Ok(())
    }

    fn upsert_element(&self, model: &mut ProjectModel, element: ArcadiaElement) -> Result<()> {
        let target_vec = self.resolve_model_vector(model, &element.kind)?;

        if let Some(pos) = target_vec.iter().position(|e| e.id == element.id) {
            target_vec[pos] = element;
        } else {
            target_vec.push(element);
        }
        Ok(())
    }

    fn delete_element(&self, model: &mut ProjectModel, id: &str) -> Result<()> {
        let all_vectors = vec![
            &mut model.oa.actors,
            &mut model.oa.activities,
            &mut model.sa.components,
            &mut model.sa.functions,
            &mut model.la.components,
            &mut model.pa.components,
        ];

        for vec in all_vectors {
            if let Some(pos) = vec.iter().position(|e| e.id == id) {
                vec.remove(pos);
                return Ok(());
            }
        }
        Err(AppError::Validation(format!(
            "Élément {} introuvable pour suppression",
            id
        )))
    }

    fn resolve_model_vector<'b>(
        &self,
        model: &'b mut ProjectModel,
        kind: &str,
    ) -> Result<&'b mut Vec<ArcadiaElement>> {
        match kind {
            "OperationalActor" => Ok(&mut model.oa.actors),
            "OperationalActivity" => Ok(&mut model.oa.activities),
            "SystemComponent" => Ok(&mut model.sa.components),
            "SystemFunction" => Ok(&mut model.sa.functions),
            "LogicalComponent" => Ok(&mut model.la.components),
            "PhysicalComponent" => Ok(&mut model.pa.components),
            _ => Err(AppError::Validation(format!(
                "Type Arcadia '{}' non géré par le ModelSync",
                kind
            ))),
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
    use crate::utils::Mutex;

    fn create_test_state() -> AppState {
        AppState {
            model: Mutex::new(ProjectModel::default()),
        }
    }

    #[test]
    fn test_upsert_new_element() {
        let state = create_test_state();
        let sync = ModelSync::new(&state);

        let mutation = Mutation {
            element_id: "urn:sa:comp1".into(),
            operation: MutationOp::Create,
            payload: json!({
                "id": "urn:sa:comp1",
                "type": "SystemComponent",
                "name": "Radar Unit"
            }),
        };

        sync.apply_mutation(&mut state.model.lock().unwrap(), &mutation)
            .unwrap();
        let model = state.model.lock().unwrap();
        assert_eq!(model.sa.components.len(), 1);
        assert_eq!(model.sa.components[0].name.as_str(), "Radar Unit");
    }

    #[test]
    fn test_delete_element_success() {
        let state = create_test_state();
        let sync = ModelSync::new(&state);

        let mut model = state.model.lock().unwrap();
        model.la.components.push(ArcadiaElement {
            id: "urn:la:ecu".into(),
            kind: "LogicalComponent".into(),
            ..Default::default()
        });
        drop(model);

        let mutation = Mutation {
            element_id: "urn:la:ecu".into(),
            operation: MutationOp::Delete,
            payload: json!({}),
        };

        sync.apply_mutation(&mut state.model.lock().unwrap(), &mutation)
            .unwrap();
        let model = state.model.lock().unwrap();
        assert!(model.la.components.is_empty());
    }
}
