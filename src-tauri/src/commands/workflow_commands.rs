// FICHIER : src-tauri/src/commands/workflow_commands.rs

use crate::workflow_engine::{
    ExecutionStatus, Mandate, WorkflowCompiler, WorkflowDefinition, WorkflowInstance,
    WorkflowScheduler,
};
// AJOUT : Import du capteur simulé (Jumeau Numérique)
use crate::workflow_engine::tools::system_tools::VIBRATION_SENSOR;

use serde::Serialize;
use std::collections::HashMap;
use tauri::{command, State};
use tokio::sync::Mutex;

/// Structure qui contient l'état global du moteur de workflow.
#[derive(Default)]
pub struct WorkflowStore {
    pub scheduler: Option<WorkflowScheduler>,
    pub instances: HashMap<String, WorkflowInstance>,
}

#[derive(Serialize)]
pub struct WorkflowView {
    pub id: String,
    pub status: ExecutionStatus,
    pub current_nodes: Vec<String>,
    pub logs: Vec<String>,
}

impl From<&WorkflowInstance> for WorkflowView {
    fn from(instance: &WorkflowInstance) -> Self {
        Self {
            id: instance.id.clone(),
            status: instance.status,
            current_nodes: instance.node_states.keys().cloned().collect(),
            logs: instance.logs.clone(),
        }
    }
}

// --- COMMANDES ---

/// NOUVELLE COMMANDE : Met à jour la valeur du capteur de vibration (Jumeau Numérique)
#[command]
pub async fn set_sensor_value(value: f64) -> Result<String, String> {
    // On verrouille le Mutex global pour mettre à jour la valeur simulée
    let mut sensor = VIBRATION_SENSOR.lock().map_err(|_| "Mutex Poisoned")?;
    *sensor = value;
    Ok(format!("Capteur mis à jour : {:.2}", value))
}

/// NOUVELLE COMMANDE : Compile et Enregistre un Mandat
#[command]
pub async fn submit_mandate(
    state: State<'_, Mutex<WorkflowStore>>,
    mandate: Mandate,
) -> Result<String, String> {
    let mut store = state.lock().await;

    // 1. (Optionnel) Ici, on vérifierait la signature avec mandate.verify_signature(...)
    // Pour l'instant, on suppose que le frontend a fait son travail.

    // 2. Compilation : Mandat (Politique) -> Workflow (Technique)
    let definition = WorkflowCompiler::compile(&mandate);
    let wf_id = definition.id.clone();

    // 3. Enregistrement dans le Scheduler
    if let Some(scheduler) = &mut store.scheduler {
        scheduler.register_workflow(definition);
        Ok(format!(
            "Mandat v{} compilé avec succès. Workflow '{}' prêt à l'exécution.",
            mandate.meta.version, wf_id
        ))
    } else {
        Err("Le moteur d'IA n'est pas encore initialisé.".to_string())
    }
}

#[command]
pub async fn register_workflow(
    state: State<'_, Mutex<WorkflowStore>>,
    definition: WorkflowDefinition,
) -> Result<String, String> {
    let mut store = state.lock().await;
    if let Some(scheduler) = &mut store.scheduler {
        let id = definition.id.clone();
        scheduler.register_workflow(definition);
        Ok(format!("Workflow '{}' enregistré avec succès.", id))
    } else {
        Err("Le moteur de workflow n'est pas encore prêt (IA en chargement).".to_string())
    }
}

#[command]
pub async fn start_workflow(
    state: State<'_, Mutex<WorkflowStore>>,
    workflow_id: String,
) -> Result<WorkflowView, String> {
    let instance_id = {
        let mut store = state.lock().await;
        if store.scheduler.is_none() {
            return Err("Le moteur de workflow n'est pas prêt.".to_string());
        }
        let instance = WorkflowInstance::new(&workflow_id, HashMap::new());
        let id = instance.id.clone();
        store.instances.insert(id.clone(), instance);
        id
    };
    run_workflow_loop(state, instance_id).await
}

#[command]
pub async fn resume_workflow(
    state: State<'_, Mutex<WorkflowStore>>,
    instance_id: String,
    node_id: String,
    approved: bool,
) -> Result<WorkflowView, String> {
    {
        let mut guard = state.lock().await;
        let WorkflowStore {
            scheduler,
            instances,
        } = &mut *guard;

        let instance = instances
            .get_mut(&instance_id)
            .ok_or("Instance introuvable")?;
        let sched = scheduler.as_ref().ok_or("Moteur non initialisé")?;

        sched
            .resume_node(instance, &node_id, approved)
            .await
            .map_err(|e| e.to_string())?;
    }
    run_workflow_loop(state, instance_id).await
}

#[command]
pub async fn get_workflow_state(
    state: State<'_, Mutex<WorkflowStore>>,
    instance_id: String,
) -> Result<WorkflowView, String> {
    let store = state.lock().await;
    let instance = store
        .instances
        .get(&instance_id)
        .ok_or("Instance introuvable")?;
    Ok(WorkflowView::from(instance))
}

async fn run_workflow_loop(
    state: State<'_, Mutex<WorkflowStore>>,
    instance_id: String,
) -> Result<WorkflowView, String> {
    loop {
        let mut guard = state.lock().await;
        let WorkflowStore {
            scheduler,
            instances,
        } = &mut *guard;

        let instance = instances
            .get_mut(&instance_id)
            .ok_or("Instance introuvable")?;
        let sched = scheduler.as_ref().ok_or("Moteur non initialisé")?;

        let keep_going = sched.run_step(instance).await.map_err(|e| e.to_string())?;

        if !keep_going {
            return Ok(WorkflowView::from(&*instance));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_not_initialized() {
        let store = WorkflowStore::default();
        assert!(store.scheduler.is_none());
        // Simulation logique
        let result = if store.scheduler.is_none() {
            Err("Moteur non prêt")
        } else {
            Ok("Succès")
        };
        assert_eq!(result, Err("Moteur non prêt"));
    }

    #[tokio::test]
    async fn test_store_initial_state() {
        let store = WorkflowStore::default();
        assert!(store.instances.is_empty());
        assert!(store.scheduler.is_none());
    }

    // --- NOUVEAUX TESTS POUR LE JUMEAU NUMÉRIQUE ---

    #[tokio::test]
    async fn test_set_sensor_value() {
        // 1. Mise à jour de la valeur
        let result = set_sensor_value(10.5).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Capteur mis à jour : 10.50");

        // 2. Vérification de l'effet de bord sur la variable globale
        {
            let lock = crate::workflow_engine::tools::system_tools::VIBRATION_SENSOR
                .lock()
                .unwrap();
            assert_eq!(*lock, 10.5);
        }

        // 3. Changement vers une valeur sûre
        let _ = set_sensor_value(2.0).await;
        {
            let lock = crate::workflow_engine::tools::system_tools::VIBRATION_SENSOR
                .lock()
                .unwrap();
            assert_eq!(*lock, 2.0);
        }
    }
}
