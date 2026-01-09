// FICHIER : src-tauri/src/commands/workflow_commands.rs

use crate::workflow_engine::{
    ExecutionStatus,
    Mandate,
    WorkflowCompiler, // AJOUT des imports
    WorkflowDefinition,
    WorkflowInstance,
    WorkflowScheduler,
};
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
}
