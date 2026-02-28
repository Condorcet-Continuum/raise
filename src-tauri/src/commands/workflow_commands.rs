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
fn create_db_manager() -> RaiseResult<(StorageEngine, String, String)> {
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
pub async fn set_sensor_value(value: f64) -> RaiseResult<String> {
    // 🎯 FIX: La commande Tauri écrit maintenant proprement dans le JsonDB (IPC par la donnée) !
    let (storage, domain, db) = create_db_manager()?;
    let manager = CollectionsManager::new(&storage, &domain, &db);

    let sensor_doc = serde_json::json!({
        "id": "vibration_z",
        "value": value,
        "updatedAt": chrono::Utc::now().to_rfc3339()
    });

    match manager.insert_raw("digital_twin", &sensor_doc).await {
        Ok(_) => (),
        Err(e) => raise_error!(
            "ERR_DT_SENSOR_WRITE_FAIL",
            error = e,
            context = json!({
                "collection": "digital_twin",
                "sensor_id": sensor_doc.get("id").unwrap_or(&json!("unknown")),
                "action": "sync_sensor_to_digital_twin",
                "hint": "Échec de l'écriture. Vérifiez l'intégrité du JSON ou les permissions du dossier 'digital_twin'."
            })
        ),
    };

    // Si on arrive ici, c'est que l'insertion a réussi
    Ok(format!("Capteur mis à jour en base : {:.2}", value))
}

#[command]
pub async fn submit_mandate(
    state: State<'_, Mutex<WorkflowStore>>,
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
    state: State<'_, Mutex<WorkflowStore>>,
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
    state: State<'_, Mutex<WorkflowStore>>,
    workflow_id: String,
) -> RaiseResult<WorkflowView> {
    let (storage, domain, db) = create_db_manager()?;
    let manager = CollectionsManager::new(&storage, &domain, &db);

    let instance_id = {
        let mut store = state.lock().await;
        if store.scheduler.is_none() {
            // 🚨 Alerte critique : Accès au moteur avant initialisation
            raise_error!(
                "ERR_WF_SCHEDULER_NOT_READY",
                context = json!({
                    "component": "scheduler_store",
                    "action": "check_engine_readiness",
                    "hint": "Le scheduler n'a pas été trouvé dans le store global. Assurez-vous que la méthode d'initialisation du moteur a été appelée avec succès avant d'interagir avec les workflows."
                })
            );
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
) -> RaiseResult<WorkflowView> {
    let (storage, domain, db) = create_db_manager()?;
    let manager = CollectionsManager::new(&storage, &domain, &db);

    {
        let mut guard = state.lock().await;
        let sched = match guard.scheduler.as_mut() {
            Some(s) => s,
            None => raise_error!(
                "ERR_ENGINE_NOT_INITIALIZED",
                context = json!({
                    "component": "scheduler",
                    "action": "execute_task",
                    "state": "uninitialized",
                    "hint": "Le moteur d'exécution (Scheduler) n'a pas été démarré. Vérifiez l'ordre d'initialisation de votre application."
                })
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
    state: State<'_, Mutex<WorkflowStore>>,
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
    state: State<'_, Mutex<WorkflowStore>>,
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
