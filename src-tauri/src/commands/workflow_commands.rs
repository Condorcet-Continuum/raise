// FICHIER : src-tauri/src/commands/workflow_commands.rs

use crate::utils::prelude::*;

use crate::workflow_engine::{
    ExecutionStatus, WorkflowCompiler, WorkflowDefinition, WorkflowInstance, WorkflowScheduler,
};

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;

use tauri::{command, State};

/// Structure qui contient l'état global du moteur de workflow.
#[derive(Default)]
pub struct WorkflowStore {
    pub scheduler: Option<WorkflowScheduler>,
    pub instances: UnorderedMap<String, WorkflowInstance>,
}

/// Vue simplifiée pour le frontend (DTO)
#[derive(Serializable)]
pub struct WorkflowView {
    pub handle: String, // 🎯 FIX : Utilisation du handle au lieu de l'id
    pub status: ExecutionStatus,
    pub current_nodes: Vec<String>,
    pub logs: Vec<String>,
}

impl From<&WorkflowInstance> for WorkflowView {
    fn from(instance: &WorkflowInstance) -> Self {
        Self {
            handle: instance.handle.clone(), // 🎯 FIX
            status: instance.status,
            current_nodes: instance.node_states.keys().cloned().collect(),
            logs: instance.logs.clone(),
        }
    }
}

// --- COMMANDES EXPOSÉES AU FRONTEND ---

/// Met à jour la valeur du capteur de vibration (Jumeau Numérique).
#[command]
pub async fn set_sensor_value(
    storage: State<'_, SharedRef<StorageEngine>>,
    value: f64,
) -> RaiseResult<String> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    internal_set_sensor(&manager, value).await
}

#[command]
pub async fn compile_mission(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mission_id: String,
) -> RaiseResult<String> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    // Compilation asynchrone via la DB
    let definition = WorkflowCompiler::compile(&manager, &mission_id).await?;
    let wf_handle = definition.handle.clone(); // 🎯 FIX : Utilisation du handle

    let mut store = state.lock().await;

    if let Some(scheduler) = &mut store.scheduler {
        scheduler.definitions.insert(wf_handle.clone(), definition);

        Ok(format!(
            "Mission '{}' compilée avec succès. Workflow '{}' prêt à l'exécution.",
            mission_id, wf_handle
        ))
    } else {
        raise_error!(
            "ERR_ENGINE_NOT_INITIALIZED",
            context = json_value!({
                "component": "scheduler",
                "workflow_handle": wf_handle,
                "mission_id": mission_id,
                "action": "register_workflow_definition",
            })
        )
    }
}

#[command]
pub async fn register_workflow(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    definition: WorkflowDefinition,
) -> RaiseResult<String> {
    let mut store = state.lock().await;
    if let Some(scheduler) = &mut store.scheduler {
        let handle = definition.handle.clone(); // 🎯 FIX
        scheduler.definitions.insert(handle.clone(), definition);

        Ok(format!("Workflow '{}' enregistré avec succès.", handle))
    } else {
        raise_error!(
            "ERR_WF_SCHEDULER_NOT_READY",
            context = json_value!({
                "action": "register_workflow_definition",
                "workflow_handle": definition.handle,
            })
        )
    }
}

#[command]
pub async fn start_workflow(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mission_id: String,
    workflow_handle: String, // 🎯 FIX : On attend un handle
) -> RaiseResult<WorkflowView> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    let instance_handle = {
        let mut store = state.lock().await;
        let scheduler = match store.scheduler.as_mut() {
            Some(s) => s,
            None => raise_error!(
                "ERR_WF_SCHEDULER_NOT_READY",
                context = json_value!({ "action": "start_workflow" })
            ),
        };

        // Création d'une instance liée à la mission
        let instance = scheduler
            .create_instance(&mission_id, &workflow_handle, &manager)
            .await?;
        let handle = instance.handle.clone(); // 🎯 FIX
        store.instances.insert(handle.clone(), instance);
        handle
    };

    run_workflow_loop(state, instance_handle, &manager).await
}

#[command]
pub async fn resume_workflow(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String, // 🎯 FIX
    node_id: String,
    approved: bool,
) -> RaiseResult<WorkflowView> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    {
        let mut guard = state.lock().await;
        let sched = match guard.scheduler.as_mut() {
            Some(s) => s,
            None => raise_error!(
                "ERR_ENGINE_NOT_INITIALIZED",
                context = json_value!({ "action": "resume_workflow" })
            ),
        };

        sched
            .resume_node(&instance_handle, &node_id, approved, &manager)
            .await?;
    }

    run_workflow_loop(state, instance_handle, &manager).await
}

#[command]
pub async fn get_workflow_state(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String, // 🎯 FIX
) -> RaiseResult<WorkflowView> {
    let store = state.lock().await;
    let instance = match store.instances.get(&instance_handle) {
        Some(inst) => inst,
        None => raise_error!(
            "ERR_CACHE_INSTANCE_NOT_FOUND",
            context = json_value!({
                "instance_handle": instance_handle,
                "action": "lookup_instance",
            })
        ),
    };
    Ok(WorkflowView::from(instance))
}

// --- HELPER : BOUCLE D'EXÉCUTION ---

async fn run_workflow_loop(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String, // 🎯 FIX
    manager: &CollectionsManager<'_>,
) -> RaiseResult<WorkflowView> {
    let final_status = {
        let guard = state.lock().await;
        let sched = match guard.scheduler.as_ref() {
            Some(s) => s,
            None => raise_error!(
                "ERR_ENGINE_NOT_INITIALIZED",
                context = json_value!({ "component": "scheduler" })
            ),
        };
        // Exécute la boucle en utilisant le handle pour le rechargement DB
        sched
            .execute_instance_loop(&instance_handle, manager)
            .await?
    };

    // Recharger l'instance mise à jour par le scheduler
    let doc = match manager
        .get_document("workflow_instances", &instance_handle)
        .await?
    {
        Some(d) => d,
        None => raise_error!(
            "ERR_WF_STATE_DESYNC",
            context = json_value!({ "instance_handle": instance_handle })
        ),
    };

    let updated_instance: WorkflowInstance = match json::deserialize_from_value(doc) {
        Ok(instance) => instance,
        Err(e) => raise_error!(
            "ERR_WORKFLOW_DESERIALIZATION_FAIL",
            error = e.to_string(),
            context = json_value!({ "instance_handle": instance_handle })
        ),
    };

    let mut store = state.lock().await;
    store
        .instances
        .insert(instance_handle.clone(), updated_instance.clone());

    tracing::info!("🏁 Boucle terminée. Statut: {:?}", final_status);
    Ok(WorkflowView::from(&updated_instance))
}

async fn internal_set_sensor(manager: &CollectionsManager<'_>, value: f64) -> RaiseResult<String> {
    let sensor_doc = json_value!({
        "handle": "vibration_z",
        "value": value,
        "updatedAt": UtcClock::now().to_rfc3339()
    });

    // 🎯 FIX : Utilisation de upsert_document (sans le &) pour l'auto-génération de l'_id !
    if let Err(e) = manager.upsert_document("digital_twin", sensor_doc).await {
        raise_error!(
            "ERR_DT_SENSOR_WRITE_FAIL",
            error = e.to_string(),
            context = json_value!({ "sensor_handle": "vibration_z" })
        );
    }

    Ok(format!("Capteur mis à jour : {:.2}", value))
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_store_not_initialized() {
        let store = WorkflowStore::default();
        assert!(store.scheduler.is_none());
    }

    #[async_test]
    async fn test_store_initial_state() {
        let store = WorkflowStore::default();
        assert!(store.instances.is_empty());
    }

    #[async_test]
    async fn test_store_lifecycle() {
        let store = WorkflowStore::default();
        assert!(store.scheduler.is_none());
        assert!(store.instances.is_empty());
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_internal_set_sensor() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        manager
            .create_collection(
                "digital_twin",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let result = internal_set_sensor(&manager, 42.0).await;
        assert!(result.is_ok());

        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(doc["value"], 42.0);
    }
}
