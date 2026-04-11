// FICHIER : src-tauri/src/commands/workflow_commands.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

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
    pub handle: String,
    pub status: ExecutionStatus,
    pub current_nodes: Vec<String>,
    pub logs: Vec<String>,
}

impl From<&WorkflowInstance> for WorkflowView {
    fn from(instance: &WorkflowInstance) -> Self {
        Self {
            handle: instance.handle.clone(),
            status: instance.status,
            current_nodes: instance.node_states.keys().cloned().collect(),
            logs: instance.logs.clone(),
        }
    }
}

// --- COMMANDES EXPOSÉES AU FRONTEND ---

/// Met à jour la valeur du capteur de vibration (Jumeau Numérique).
/// Utilise les points de montage pour la résolution sémantique.
#[command]
pub async fn set_sensor_value(
    storage: State<'_, SharedRef<StorageEngine>>,
    value: f64,
) -> RaiseResult<String> {
    let config = AppConfig::get();
    // 🎯 RÉSILIENCE : Résolution via Mount Points
    let manager = CollectionsManager::new(
        &storage,
        &config.mount_points.system.domain,
        &config.mount_points.system.db,
    );

    internal_set_sensor(&manager, value).await
}

#[command]
pub async fn compile_mission(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mission_id: String,
) -> RaiseResult<String> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(
        &storage,
        &config.mount_points.system.domain,
        &config.mount_points.system.db,
    );

    // Compilation asynchrone avec Match strict
    let definition = match WorkflowCompiler::compile(&manager, &mission_id).await {
        Ok(d) => d,
        Err(e) => raise_error!(
            "ERR_WF_COMPILATION_FAIL",
            error = e.to_string(),
            context = json_value!({"mission_id": mission_id})
        ),
    };

    let wf_handle = definition.handle.clone();
    let mut store = state.lock().await;

    match &mut store.scheduler {
        Some(scheduler) => {
            scheduler.definitions.insert(wf_handle.clone(), definition);
            Ok(format!(
                "Mission '{}' compilée. Workflow '{}' prêt.",
                mission_id, wf_handle
            ))
        }
        None => raise_error!(
            "ERR_WF_SCHEDULER_NOT_READY",
            context = json_value!({"mission_id": mission_id})
        ),
    }
}

#[command]
pub async fn register_workflow(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    definition: WorkflowDefinition,
) -> RaiseResult<String> {
    let mut store = state.lock().await;
    let handle = definition.handle.clone();

    match &mut store.scheduler {
        Some(scheduler) => {
            scheduler.definitions.insert(handle.clone(), definition);
            Ok(format!("Workflow '{}' enregistré avec succès.", handle))
        }
        None => raise_error!(
            "ERR_WF_SCHEDULER_NOT_READY",
            context = json_value!({"workflow": handle})
        ),
    }
}

#[command]
pub async fn start_workflow(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mission_id: String,
    workflow_handle: String,
) -> RaiseResult<WorkflowView> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(
        &storage,
        &config.mount_points.system.domain,
        &config.mount_points.system.db,
    );

    let instance_handle = {
        let mut store = state.lock().await;
        let scheduler = match store.scheduler.as_mut() {
            Some(s) => s,
            None => raise_error!("ERR_WF_SCHEDULER_NOT_READY"),
        };

        let instance = scheduler
            .create_instance(&mission_id, &workflow_handle, &manager)
            .await?;
        let handle = instance.handle.clone();
        store.instances.insert(handle.clone(), instance);
        handle
    };

    run_workflow_loop(state, instance_handle, &manager).await
}

#[command]
pub async fn resume_workflow(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String,
    node_id: String,
    approved: bool,
) -> RaiseResult<WorkflowView> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(
        &storage,
        &config.mount_points.system.domain,
        &config.mount_points.system.db,
    );

    {
        let mut guard = state.lock().await;
        let sched = match guard.scheduler.as_mut() {
            Some(s) => s,
            None => raise_error!("ERR_WF_SCHEDULER_NOT_READY"),
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
    instance_handle: String,
) -> RaiseResult<WorkflowView> {
    let store = state.lock().await;
    match store.instances.get(&instance_handle) {
        Some(inst) => Ok(WorkflowView::from(inst)),
        None => raise_error!(
            "ERR_WF_INSTANCE_NOT_FOUND",
            context = json_value!({ "handle": instance_handle })
        ),
    }
}

// --- HELPER : BOUCLE D'EXÉCUTION ---

async fn run_workflow_loop(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String,
    manager: &CollectionsManager<'_>,
) -> RaiseResult<WorkflowView> {
    let _final_status = {
        let guard = state.lock().await;
        match guard.scheduler.as_ref() {
            Some(s) => s.execute_instance_loop(&instance_handle, manager).await?,
            None => raise_error!("ERR_WF_SCHEDULER_NOT_READY"),
        }
    };

    // Rechargement résilient de l'instance
    let doc = match manager
        .get_document("workflow_instances", &instance_handle)
        .await?
    {
        Some(d) => d,
        None => raise_error!(
            "ERR_WF_STATE_DESYNC",
            context = json_value!({ "handle": instance_handle })
        ),
    };

    let updated_instance: WorkflowInstance = match json::deserialize_from_value(doc) {
        Ok(instance) => instance,
        Err(e) => raise_error!("ERR_WF_DESERIALIZATION_FAIL", error = e.to_string()),
    };

    let mut store = state.lock().await;
    store
        .instances
        .insert(instance_handle.clone(), updated_instance.clone());

    Ok(WorkflowView::from(&updated_instance))
}

async fn internal_set_sensor(manager: &CollectionsManager<'_>, value: f64) -> RaiseResult<String> {
    let sensor_doc = json_value!({
        "handle": "vibration_z",
        "value": value,
        "updatedAt": UtcClock::now().to_rfc3339()
    });

    match manager.upsert_document("digital_twin", sensor_doc).await {
        Ok(_) => Ok(format!("Capteur mis à jour : {:.2}", value)),
        Err(e) => raise_error!("ERR_DT_SENSOR_WRITE_FAIL", error = e.to_string()),
    }
}

// --- TESTS UNITAIRES ET RÉSILIENCE ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    async fn run_workflow_loop_internal(
        state: &AsyncMutex<WorkflowStore>,
        instance_handle: String,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<WorkflowView> {
        {
            let guard = state.lock().await;
            match guard.scheduler.as_ref() {
                Some(s) => s.execute_instance_loop(&instance_handle, manager).await?,
                None => raise_error!("ERR_WF_SCHEDULER_NOT_READY"),
            };
        }

        // Rechargement résilient après exécution de la boucle
        let doc = match manager
            .get_document("workflow_instances", &instance_handle)
            .await?
        {
            Some(d) => d,
            None => raise_error!(
                "ERR_WF_STATE_DESYNC",
                context = json_value!({ "handle": instance_handle })
            ),
        };

        let updated: WorkflowInstance = match json::deserialize_from_value(doc) {
            Ok(i) => i,
            Err(e) => raise_error!("ERR_WF_DESERIALIZATION_FAIL", error = e.to_string()),
        };

        let mut store = state.lock().await;
        store
            .instances
            .insert(instance_handle.clone(), updated.clone());

        Ok(WorkflowView::from(&updated))
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_initial_state() -> RaiseResult<()> {
        let store = WorkflowStore::default();
        assert!(store.scheduler.is_none());
        assert!(store.instances.is_empty());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_internal_set_sensor() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            manager.space, manager.db
        );
        manager
            .create_collection("digital_twin", &schema_uri)
            .await?;

        internal_set_sensor(&manager, 42.0).await?;

        let doc = manager
            .get_document("digital_twin", "vibration_z")
            .await?
            .unwrap();
        assert_eq!(doc["value"], 42.0);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une désynchronisation de l'instance
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_instance_desync() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // Initialisation du manager via les points de montage système
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Création d'un store vide (Scheduler non initialisé)
        let state = AsyncMutex::new(WorkflowStore::default());

        // On appelle la logique interne directement pour éviter l'anti-pattern State::Boxed
        let res = run_workflow_loop_internal(&state, "ghost_handle".into(), &manager).await;

        // Validation de la résilience du moteur face à un scheduler manquant
        match res {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_WF_SCHEDULER_NOT_READY");
                Ok(())
            }
            _ => panic!(
                "Le moteur aurait dû lever ERR_WF_SCHEDULER_NOT_READY car le scheduler est None"
            ),
        }
    }
}
