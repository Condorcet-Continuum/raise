// FICHIER : src-tauri/src/commands/workflow_commands.rs

use crate::utils::{prelude::*, Arc, AsyncMutex, HashMap};

use crate::workflow_engine::{
    ExecutionStatus, Mandate, WorkflowCompiler, WorkflowDefinition, WorkflowInstance,
    WorkflowScheduler,
};

// 🎯 FIX: Suppression de l'import du VIBRATION_SENSOR (Mutex supprimé)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;

use tauri::{command, State};

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

// --- COMMANDES EXPOSÉES AU FRONTEND ---

/// Met à jour la valeur du capteur de vibration (Jumeau Numérique).
#[command]
pub async fn set_sensor_value(
    storage: State<'_, Arc<StorageEngine>>,
    value: f64,
) -> RaiseResult<String> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    internal_set_sensor(&manager, value).await
}

#[command]
pub async fn submit_mandate(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mandate: Mandate,
) -> RaiseResult<String> {
    let mut store = state.lock().await;

    let definition = WorkflowCompiler::compile(&mandate);
    let wf_id = definition.id.clone();

    if let Some(scheduler) = &mut store.scheduler {
        scheduler.definitions.insert(wf_id.clone(), definition);

        // Succès : On renvoie le message formatté
        Ok(format!(
            "Mandat v{} compilé avec succès. Workflow '{}' prêt à l'exécution.",
            mandate.meta.version, wf_id
        ))
    } else {
        // ⚠️ Erreur d'état du moteur
        raise_error!(
            "ERR_ENGINE_NOT_INITIALIZED",
            context = json!({
                "component": "scheduler",
                "workflow_id": wf_id,
                "mandate_version": mandate.meta.version,
                "action": "register_workflow_definition",
                "hint": "Le scheduler est manquant dans le store. Vérifiez que le moteur d'IA a bien été démarré avant de compiler des mandats."
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
        let id = definition.id.clone();
        scheduler.definitions.insert(id.clone(), definition);

        // Succès : On renvoie une confirmation claire
        Ok(format!("Workflow '{}' enregistré avec succès.", id))
    } else {
        // ⚠️ Erreur de cycle de vie du système
        raise_error!(
            "ERR_WF_SCHEDULER_NOT_READY",
            context = json!({
                "action": "register_workflow_definition",
                "workflow_id": definition.id,
                "component": "scheduler_store",
                "hint": "Le scheduler est manquant dans le store. L'initialisation du moteur a probablement échoué ou n'est pas encore terminée."
            })
        )
    }
}

#[command]
pub async fn start_workflow(
    storage: State<'_, Arc<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    workflow_id: String,
) -> RaiseResult<WorkflowView> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

    let instance_id = {
        let mut store = state.lock().await;
        let scheduler = match store.scheduler.as_mut() {
            Some(s) => s,
            None => raise_error!(
                "ERR_WF_SCHEDULER_NOT_READY",
                context = json!({ "action": "start_workflow" })
            ),
        };

        let instance = scheduler.create_instance(&workflow_id, &manager).await?;
        let id = instance.id.clone();
        store.instances.insert(id.clone(), instance);
        id
    };

    run_workflow_loop(state, instance_id, &manager).await
}

#[command]
pub async fn resume_workflow(
    storage: State<'_, Arc<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_id: String,
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
                context = json!({ "action": "resume_workflow" })
            ),
        };

        sched
            .resume_node(&instance_id, &node_id, approved, &manager)
            .await?;
    }

    run_workflow_loop(state, instance_id, &manager).await
}

#[command]
pub async fn get_workflow_state(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_id: String,
) -> RaiseResult<WorkflowView> {
    let store = state.lock().await;
    let instance = match store.instances.get(&instance_id) {
        Some(inst) => inst,
        None => raise_error!(
            "ERR_CACHE_INSTANCE_NOT_FOUND",
            context = json!({
                "instance_id": instance_id,
                "cache_type": "wasm_instance_store",
                "action": "lookup_instance",
                "hint": format!("L'instance '{}' n'existe pas ou a été purgée du cache. Vérifiez si le plugin a été correctement chargé.", instance_id)
            })
        ),
    };
    Ok(WorkflowView::from(instance))
}

// --- HELPER : BOUCLE D'EXÉCUTION SOUVERAINE ---

async fn run_workflow_loop(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_id: String,
    manager: &CollectionsManager<'_>,
) -> RaiseResult<WorkflowView> {
    let final_status = {
        let guard = state.lock().await;
        let sched = match guard.scheduler.as_ref() {
            Some(s) => s,
            None => raise_error!(
                "ERR_ENGINE_NOT_INITIALIZED",
                context = json!({
                    "component": "scheduler",
                    "access_mode": "read_only",
                    "state": "uninitialized",
                    "hint": "Tentative d'accès au Scheduler en lecture alors qu'il n'est pas initialisé. Vérifiez le flux de démarrage d'Arcadia."
                })
            ),
        };
        sched.execute_instance_loop(&instance_id, manager).await?
    };

    // 1. On tente la lecture avec un match explicite pour le Result
    let doc_opt = match manager
        .get_document("workflow_instances", &instance_id)
        .await
    {
        Ok(d) => d,
        Err(e) => raise_error!(
            "ERR_WF_POST_EXEC_READ_FAIL",
            error = e,
            context = json!({
                "instance_id": instance_id,
                "action": "post_execution_state_sync"
            })
        ),
    };

    // 2. On gère le cas où le document n'existe pas (Option) avec un second match
    let doc = match doc_opt {
        Some(d) => d,
        None => raise_error!(
            "ERR_WF_STATE_DESYNC",
            context = json!({
                "instance_id": instance_id,
                "action": "verify_final_state",
                "hint": "L'instance a disparu après l'exécution. Vérifiez si une suppression concurrente ou un rollback a eu lieu."
            })
        ),
    };

    let updated_instance: WorkflowInstance = match serde_json::from_value(doc.clone()) {
        Ok(instance) => instance,
        Err(e) => raise_error!(
            "ERR_WORKFLOW_DESERIALIZATION_FAIL",
            error = e,
            context = json!({
                "action": "update_workflow_instance",
                "document_snapshot": doc,
                "hint": "Le document JSON ne correspond pas à la structure WorkflowInstance. Vérifiez les champs obligatoires et le typage des enums."
            })
        ),
    };

    let mut store = state.lock().await;
    store
        .instances
        .insert(instance_id.clone(), updated_instance.clone());

    tracing::info!("🏁 Boucle frontend terminée. Statut: {:?}", final_status);
    Ok(WorkflowView::from(&updated_instance))
}

async fn internal_set_sensor(manager: &CollectionsManager<'_>, value: f64) -> RaiseResult<String> {
    let sensor_doc = serde_json::json!({
        "id": "vibration_z",
        "value": value,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    });

    if let Err(e) = manager.insert_raw("digital_twin", &sensor_doc).await {
        raise_error!(
            "ERR_DT_SENSOR_WRITE_FAIL",
            error = e,
            context = serde_json::json!({ "sensor_id": "vibration_z" })
        );
    }

    Ok(format!("Capteur mis à jour : {:.2}", value))
}
// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::test_mocks::AgentDbSandbox;

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
    async fn test_store_lifecycle() {
        let store = WorkflowStore::default();
        assert!(store.scheduler.is_none());
        assert!(store.instances.is_empty());
    }

    #[tokio::test]
    #[serial_test::serial]
    async fn test_internal_set_sensor() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 🎯 Test de la logique pure
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
