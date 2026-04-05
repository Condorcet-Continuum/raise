// FICHIER : src-tauri/src/workflow_engine/scheduler.rs
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use crate::workflow_engine::{
    executor::WorkflowExecutor, state_machine::WorkflowStateMachine, ExecutionStatus,
    WorkflowDefinition, WorkflowInstance,
};

pub struct WorkflowScheduler {
    pub executor: WorkflowExecutor,
    pub definitions: UnorderedMap<String, WorkflowDefinition>,
}

impl WorkflowScheduler {
    pub fn new(executor: WorkflowExecutor) -> Self {
        Self {
            executor,
            definitions: UnorderedMap::new(),
        }
    }

    pub async fn load_mission<'a>(
        &mut self,
        mission_handle: &str,
        manager: &'a CollectionsManager<'a>,
    ) -> RaiseResult<()> {
        tracing::info!("📥 Chargement de la mission : {}", mission_handle);
        let workflow = WorkflowExecutor::load_and_prepare_workflow(manager, mission_handle).await?;

        // 🎯 FIX : Utilisation du handle comme clé de stockage (plus l'id)
        self.definitions.insert(workflow.handle.clone(), workflow);
        Ok(())
    }

    pub async fn create_instance<'a>(
        &self,
        mission_id: &str,
        workflow_handle: &str, // Passage au handle sémantique
        manager: &'a CollectionsManager<'a>,
    ) -> RaiseResult<WorkflowInstance> {
        let def = match self.definitions.get(workflow_handle) {
            Some(definition) => definition,
            None => raise_error!(
                "ERR_WF_DEFINITION_NOT_FOUND",
                context = json_value!({"workflow_handle": workflow_handle})
            ),
        };

        // 🎯 FIX : Initialisation handle-based sans forcer l'_id
        let mut instance = WorkflowInstance {
            _id: None, // Laissé à la gestion interne de json_db
            handle: format!(
                "inst_{}_{}",
                workflow_handle,
                UtcClock::now().timestamp_millis()
            ),
            mission_id: mission_id.to_string(),
            workflow_id: def.handle.clone(),
            status: ExecutionStatus::Pending,
            node_states: UnorderedMap::new(),
            context: UnorderedMap::new(),
            xai_traces: Vec::new(),
            logs: vec![format!(
                "Création de l'instance pour le workflow {}",
                def.handle
            )],
            created_at: UtcClock::now().timestamp(),
            updated_at: UtcClock::now().timestamp(),
        };

        self.persist_instance(&mut instance, manager).await?;
        Ok(instance)
    }

    pub async fn run_step<'a>(
        &'a self,
        instance: &mut WorkflowInstance,
        manager: &'a CollectionsManager<'a>,
    ) -> RaiseResult<bool> {
        // 🎯 Recherche de la définition par son handle métier
        let def = match self.definitions.get(&instance.workflow_id) {
            Some(d) => d,
            None => raise_error!(
                "ERR_WF_INSTANCE_ORPHAN",
                context = json_value!({"instance_handle": instance.handle})
            ),
        };
        let sm = WorkflowStateMachine::new(def);
        let runnable_nodes = sm.next_runnable_nodes(instance).await;

        if runnable_nodes.is_empty() {
            if instance.status == ExecutionStatus::Running {
                instance.status = ExecutionStatus::Completed;
                instance
                    .logs
                    .push("🏁 Exécution terminée avec succès.".into());
                self.persist_instance(instance, manager).await?;
            }
            return Ok(false);
        }

        instance.status = ExecutionStatus::Running;
        let mut progress_made = false;

        for node_id in runnable_nodes {
            if let Some(node) = def.nodes.iter().find(|n| n.id == node_id) {
                let status = self
                    .executor
                    .execute_node(node, &mut instance.context, manager)
                    .await?;

                if let Err(e) = sm.transition(instance, &node_id, status) {
                    raise_error!("ERR_WF_STATE_TRANSITION_FAILED", error = e.to_string());
                }

                instance
                    .logs
                    .push(format!("⚙️ Nœud '{}' -> {:?}", node.name, status));
                progress_made = true;

                if status == ExecutionStatus::Paused || status == ExecutionStatus::Failed {
                    instance.status = status;
                    break;
                }
            }
        }

        if progress_made {
            self.persist_instance(instance, manager).await?;
        }

        Ok(progress_made)
    }

    pub async fn execute_instance_loop<'a>(
        &'a self,
        instance_handle: &str, // Utilisation du handle pour la recherche
        manager: &'a CollectionsManager<'a>,
    ) -> RaiseResult<ExecutionStatus> {
        let doc = match manager
            .get_document("workflow_instances", instance_handle)
            .await?
        {
            Some(d) => d,
            None => raise_error!(
                "ERR_WF_INSTANCE_NOT_FOUND",
                context = json_value!({"instance_handle": instance_handle})
            ),
        };

        let mut instance: WorkflowInstance = match json::deserialize_from_value(doc) {
            Ok(inst) => inst,
            Err(e) => raise_error!(
                "ERR_WORKFLOW_DESERIALIZATION_FAIL",
                error = e.to_string(),
                context = json_value!({"instance_handle": instance_handle})
            ),
        };

        loop {
            if !self.run_step(&mut instance, manager).await? {
                break;
            }
        }

        Ok(instance.status)
    }

    pub async fn resume_node<'a>(
        &self,
        instance_handle: &str,
        node_id: &str,
        approved: bool,
        manager: &'a CollectionsManager<'a>,
    ) -> RaiseResult<ExecutionStatus> {
        let doc = match manager
            .get_document("workflow_instances", instance_handle)
            .await?
        {
            Some(d) => d,
            None => raise_error!(
                "ERR_WF_INSTANCE_NOT_FOUND",
                context = json_value!({"instance_handle": instance_handle})
            ),
        };
        let mut instance: WorkflowInstance = json::deserialize_from_value(doc).unwrap();

        let new_status = if approved {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };
        instance.node_states.insert(node_id.to_string(), new_status);
        instance.status = ExecutionStatus::Running;

        self.persist_instance(&mut instance, manager).await?;
        Ok(instance.status)
    }

    async fn persist_instance(
        &self,
        instance: &mut WorkflowInstance,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<()> {
        instance.updated_at = UtcClock::now().timestamp();
        let json_val = json::serialize_to_value(&instance).unwrap();
        manager
            .upsert_document("workflow_instances", json_val)
            .await?;
        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::{NodeType, WorkflowEdge, WorkflowNode};

    async fn setup_test_environment(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &AppConfig,
    ) -> WorkflowScheduler {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);
        manager
            .create_collection(
                "workflow_instances",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, storage.clone())
            .await
            .unwrap();
        let pm = SharedRef::new(PluginManager::new(&storage, None));
        let executor = WorkflowExecutor::new(SharedRef::new(AsyncMutex::new(orch)), pm);

        WorkflowScheduler::new(executor)
    }

    fn create_mock_workflow(
        handle_name: &str,
        nodes: Vec<WorkflowNode>,
        edges: Vec<WorkflowEdge>,
    ) -> WorkflowDefinition {
        let entry = nodes.first().map(|n| n.id.clone()).unwrap_or_default();
        WorkflowDefinition {
            _id: None,                       // 🎯 FIX : Initialisation obligatoire
            handle: handle_name.to_string(), // 🎯 FIX : Remplacement d'id par handle
            entry,
            nodes,
            edges,
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_create_instance_and_persistence() {
        let sandbox = AgentDbSandbox::new().await;
        let mut scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let def = create_mock_workflow("wf_empty", vec![], vec![]);
        scheduler.definitions.insert("wf_empty".to_string(), def);

        let instance = scheduler
            .create_instance("mission_test", "wf_empty", &manager)
            .await
            .expect("Échec création");

        // 🎯 FIX : Vérification basée sur le handle métier
        assert_eq!(instance.workflow_id, "wf_empty");
        assert_eq!(instance.status, ExecutionStatus::Pending);

        let doc = manager
            .get_document("workflow_instances", &instance.handle)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(doc["workflowId"], "wf_empty");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_scheduler_step_by_step_execution() {
        let sandbox = AgentDbSandbox::new().await;
        let mut scheduler = setup_test_environment(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let n_start = WorkflowNode {
            id: "n1".into(),
            r#type: NodeType::End,
            name: "Start".into(),
            params: JsonValue::Null,
        };
        let def = create_mock_workflow("wf_mini", vec![n_start], vec![]);
        scheduler.definitions.insert("wf_mini".to_string(), def);

        let mut instance = scheduler
            .create_instance("mission_test", "wf_mini", &manager)
            .await
            .unwrap();

        let progress = scheduler.run_step(&mut instance, &manager).await.unwrap();
        assert!(progress);
        assert_eq!(instance.status, ExecutionStatus::Completed);
    }
}
