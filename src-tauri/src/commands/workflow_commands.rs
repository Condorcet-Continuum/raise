// FICHIER : src-tauri/src/commands/workflow_commands.rs

use crate::utils::{prelude::*, HashMap};

use crate::workflow_engine::{
    ExecutionStatus, Mandate, WorkflowCompiler, WorkflowDefinition, WorkflowInstance,
    WorkflowScheduler,
};

// 🎯 FIX: Suppression de l'import du VIBRATION_SENSOR (Mutex supprimé)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use std::path::PathBuf;

use tauri::{command, State};
use tokio::sync::Mutex;

/// Structure qui contient l'état global du moteur de workflow.
#[derive(Default)]
pub struct WorkflowStore {
    pub scheduler: Option<WorkflowScheduler>,
    pub instances: HashMap<String, WorkflowInstance>,
}

/// Vue simplifiée pour le frontend (DTO)
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

// --- HELPER DB ---
fn create_db_manager() -> Result<(StorageEngine, String, String)> {
    let config = AppConfig::get();
    let path = config
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_else(|| PathBuf::from("./_system"));

    let storage = StorageEngine::new(JsonDbConfig::new(path));
    Ok((
        storage,
        config.system_domain.clone(),
        config.system_db.clone(),
    ))
}

// --- COMMANDES EXPOSÉES AU FRONTEND ---

/// Met à jour la valeur du capteur de vibration (Jumeau Numérique).
#[command]
pub async fn set_sensor_value(value: f64) -> Result<String> {
    // 🎯 FIX: La commande Tauri écrit maintenant proprement dans le JsonDB (IPC par la donnée) !
    let (storage, domain, db) = create_db_manager()?;
    let manager = CollectionsManager::new(&storage, &domain, &db);

    let sensor_doc = serde_json::json!({
        "id": "vibration_z",
        "value": value,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    });

    manager
        .insert_raw("digital_twin", &sensor_doc)
        .await
        .map_err(|e| AppError::Database(format!("Erreur d'écriture capteur: {}", e)))?;

    Ok(format!("Capteur mis à jour en base : {:.2}", value))
}

#[command]
pub async fn submit_mandate(
    state: State<'_, Mutex<WorkflowStore>>,
    mandate: Mandate,
) -> Result<String> {
    let mut store = state.lock().await;

    let definition = WorkflowCompiler::compile(&mandate);
    let wf_id = definition.id.clone();

    if let Some(scheduler) = &mut store.scheduler {
        scheduler.definitions.insert(wf_id.clone(), definition);
        Ok(format!(
            "Mandat v{} compilé avec succès. Workflow '{}' prêt à l'exécution.",
            mandate.meta.version, wf_id
        ))
    } else {
        Err(AppError::from(
            "Le moteur d'IA n'est pas encore initialisé.".to_string(),
        ))
    }
}

#[command]
pub async fn register_workflow(
    state: State<'_, Mutex<WorkflowStore>>,
    definition: WorkflowDefinition,
) -> Result<String> {
    let mut store = state.lock().await;
    if let Some(scheduler) = &mut store.scheduler {
        let id = definition.id.clone();
        scheduler.definitions.insert(id.clone(), definition);
        Ok(format!("Workflow '{}' enregistré avec succès.", id))
    } else {
        Err(AppError::from(
            "Le moteur de workflow n'est pas encore prêt.".to_string(),
        ))
    }
}

#[command]
pub async fn start_workflow(
    state: State<'_, Mutex<WorkflowStore>>,
    workflow_id: String,
) -> Result<WorkflowView> {
    let (storage, domain, db) = create_db_manager()?;
    let manager = CollectionsManager::new(&storage, &domain, &db);

    let instance_id = {
        let mut store = state.lock().await;
        if store.scheduler.is_none() {
            return Err(AppError::from("Le moteur de workflow n'est pas prêt."));
        }

        let scheduler = store.scheduler.as_mut().unwrap();
        let instance = scheduler.create_instance(&workflow_id, &manager).await?;
        let id = instance.id.clone();
        store.instances.insert(id.clone(), instance);
        id
    };

    run_workflow_loop(state, instance_id, &manager).await
}

#[command]
pub async fn resume_workflow(
    state: State<'_, Mutex<WorkflowStore>>,
    instance_id: String,
    node_id: String,
    approved: bool,
) -> Result<WorkflowView> {
    let (storage, domain, db) = create_db_manager()?;
    let manager = CollectionsManager::new(&storage, &domain, &db);

    {
        let mut guard = state.lock().await;
        let sched = guard
            .scheduler
            .as_mut()
            .ok_or_else(|| AppError::from("Moteur non initialisé".to_string()))?;

        sched
            .resume_node(&instance_id, &node_id, approved, &manager)
            .await?;
    }

    run_workflow_loop(state, instance_id, &manager).await
}

#[command]
pub async fn get_workflow_state(
    state: State<'_, Mutex<WorkflowStore>>,
    instance_id: String,
) -> Result<WorkflowView> {
    let store = state.lock().await;
    let instance = store
        .instances
        .get(&instance_id)
        .ok_or_else(|| AppError::from("Instance introuvable en cache".to_string()))?;
    Ok(WorkflowView::from(instance))
}

// --- HELPER : BOUCLE D'EXÉCUTION SOUVERAINE ---

async fn run_workflow_loop(
    state: State<'_, Mutex<WorkflowStore>>,
    instance_id: String,
    manager: &CollectionsManager<'_>,
) -> Result<WorkflowView> {
    let final_status = {
        let guard = state.lock().await;
        let sched = guard
            .scheduler
            .as_ref()
            .ok_or_else(|| AppError::from("Moteur non initialisé".to_string()))?;
        sched.execute_instance_loop(&instance_id, manager).await?
    };

    let doc = manager
        .get_document("workflow_instances", &instance_id)
        .await
        .map_err(|e| AppError::Database(e.to_string()))?
        .ok_or_else(|| {
            AppError::NotFound("Instance introuvable en base après exécution".to_string())
        })?;

    let updated_instance: WorkflowInstance =
        serde_json::from_value(doc).map_err(AppError::Serialization)?;

    let mut store = state.lock().await;
    store
        .instances
        .insert(instance_id.clone(), updated_instance.clone());

    tracing::info!("🏁 Boucle frontend terminée. Statut: {:?}", final_status);
    Ok(WorkflowView::from(&updated_instance))
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_store_not_initialized() {
        let store = WorkflowStore::default();
        assert!(store.scheduler.is_none());
    }

    #[tokio::test]
    async fn test_store_initial_state() {
        let store = WorkflowStore::default();
        assert!(store.instances.is_empty());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_set_sensor_value() {
        crate::utils::config::test_mocks::inject_mock_config();

        // 1. Mise à jour de la valeur via la commande
        let result = set_sensor_value(10.5).await;
        assert!(result.is_ok());

        // 2. Vérification dans JsonDB (IPC validé)
        let (storage, domain, db) = create_db_manager().unwrap();
        let manager = CollectionsManager::new(&storage, &domain, &db);

        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(doc["value"], 10.5);
    }
}
