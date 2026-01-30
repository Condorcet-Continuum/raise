// FICHIER : src-tauri/src/workflow_engine/scheduler.rs

use super::{
    executor::WorkflowExecutor, state_machine::WorkflowStateMachine, ExecutionStatus,
    WorkflowDefinition, WorkflowInstance,
};
use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::collections::manager::CollectionsManager;
// AJOUT : Import du manager de plugins pour la cohérence avec l'executor
use crate::plugins::manager::PluginManager;
use crate::utils::Result;

use super::tools::{AgentTool, SystemMonitorTool};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Le Scheduler pilote l'exécution des workflows et assure le pont avec l'IA.
pub struct WorkflowScheduler {
    pub executor: WorkflowExecutor,
    pub definitions: HashMap<String, WorkflowDefinition>,
    pub orchestrator: Arc<Mutex<AiOrchestrator>>,
    /// Référence vers le gestionnaire de plugins
    pub plugin_manager: Arc<PluginManager>,
}

impl WorkflowScheduler {
    pub fn new(
        orchestrator: Arc<Mutex<AiOrchestrator>>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        // CORRECTION E0061 : passage du deuxième argument requis par WorkflowExecutor::new
        let mut executor = WorkflowExecutor::new(orchestrator.clone(), plugin_manager.clone());
        executor.register_tool(Box::new(SystemMonitorTool));

        Self {
            executor,
            definitions: HashMap::new(),
            orchestrator,
            plugin_manager,
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn AgentTool>) {
        self.executor.register_tool(tool);
    }

    pub fn register_workflow(&mut self, def: WorkflowDefinition) {
        self.definitions.insert(def.id.clone(), def);
    }

    /// Charge une mission complète à partir d'un Mandat stocké en base.
    /// C'est le point d'entrée "Politique -> Technique".
    pub async fn load_mission(
        &mut self,
        manager: &CollectionsManager<'_>,
        mandate_id: &str,
    ) -> Result<String> {
        // 1. Appel au Pont (Executor) pour transformer le Mandat en Workflow
        let workflow = WorkflowExecutor::load_and_prepare_workflow(manager, mandate_id).await?;

        let wf_id = workflow.id.clone();
        tracing::info!("🚀 Mission chargée dans le Scheduler : {}", wf_id);

        // 2. Enregistrement en mémoire
        self.register_workflow(workflow);

        Ok(wf_id)
    }

    pub async fn run_step(&self, instance: &mut WorkflowInstance) -> Result<bool> {
        let def = self.definitions.get(&instance.workflow_id).ok_or_else(|| {
            crate::utils::AppError::NotFound(format!(
                "Workflow def '{}' not found",
                instance.workflow_id
            ))
        })?;

        let sm = WorkflowStateMachine::new(def.clone());
        let runnable_nodes = sm.next_runnable_nodes(instance);

        if runnable_nodes.is_empty() {
            return Ok(false);
        }

        instance.status = ExecutionStatus::Running;
        let mut progress_made = false;

        for node_id in runnable_nodes {
            if let Some(node) = def.nodes.iter().find(|n| n.id == node_id) {
                let status = self
                    .executor
                    .execute_node(node, &mut instance.context)
                    .await?;

                sm.transition(instance, &node_id, status)
                    .map_err(|e| crate::utils::AppError::from(e.to_string()))?;

                if status == ExecutionStatus::Paused {
                    instance.status = ExecutionStatus::Paused;
                    return Ok(false);
                }

                progress_made = true;
            }
        }

        Ok(progress_made)
    }

    pub async fn resume_node(
        &self,
        instance: &mut WorkflowInstance,
        node_id: &str,
        approved: bool,
    ) -> Result<()> {
        let new_status = if approved {
            ExecutionStatus::Completed
        } else {
            ExecutionStatus::Failed
        };

        if let Some(state) = instance.node_states.get_mut(node_id) {
            *state = new_status;
        } else {
            instance.node_states.insert(node_id.to_string(), new_status);
        }

        instance.status = ExecutionStatus::Running;
        instance
            .logs
            .push(format!("⏩ Reprise après validation du nœud '{}'", node_id));

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::json_db::test_utils::init_test_env;
    use crate::model_engine::types::ProjectModel;
    // WorkflowNode est importé ici pour éviter le warning unused_import en haut du fichier
    use crate::workflow_engine::{
        ExecutionStatus, NodeType, WorkflowDefinition, WorkflowEdge, WorkflowNode,
    };
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::sync::Mutex;

    async fn setup_test() -> WorkflowScheduler {
        let model = ProjectModel::default();
        // CORRECTION E0061 : Ajout de None pour l'argument StorageEngine (pas nécessaire dans ce mock)
        let orch = AiOrchestrator::new(
            model,
            "http://127.0.0.1:6334",
            "http://127.0.0.1:8081",
            None,
        )
        .await
        .unwrap_or_else(|_| panic!("Mock fail"));

        let shared_orch = Arc::new(Mutex::new(orch));

        // Initialisation de la dépendance PluginManager pour les tests
        let dir = tempdir().unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(dir.path().to_path_buf()));
        let plugin_manager = Arc::new(PluginManager::new(&storage, None));

        WorkflowScheduler::new(shared_orch, plugin_manager)
    }

    #[tokio::test]
    #[ignore = "Nécessite AiOrchestrator (Llama.cpp/Qdrant)"]
    async fn test_mission_lifecycle_from_mandate() {
        let mut scheduler = setup_test().await;
        let env = init_test_env().await;
        let manager = CollectionsManager::new(&env.storage, &env.space, &env.db);

        let mandate = json!({
            "id": "mission_alpha",
            "meta": { "author": "Commander", "version": "1.0", "status": "ACTIVE" },
            "governance": { "strategy": "PERFORMANCE" },
            "hardLogic": { "vetos": [] },
            "observability": { "heartbeatMs": 1000 }
        });
        manager.insert_raw("mandates", &mandate).await.unwrap();

        let workflow_id = scheduler
            .load_mission(&manager, "mission_alpha")
            .await
            .expect("Chargement échoué");

        assert_eq!(workflow_id, "wf_Commander_1.0");
        assert!(scheduler.definitions.contains_key(&workflow_id));

        let mut instance = WorkflowInstance::new(&workflow_id, HashMap::new());

        let result = scheduler.run_step(&mut instance).await;
        assert!(result.is_ok());
        assert_eq!(instance.status, ExecutionStatus::Running);

        assert!(instance.node_states.contains_key("start"));
    }

    #[tokio::test]
    #[ignore = "Nécessite AiOrchestrator (Llama.cpp/Qdrant)"]
    async fn test_full_agentic_loop() {
        let mut scheduler = setup_test().await;

        let def = WorkflowDefinition {
            id: "wf_test_hitl".into(),
            entry: "node_1".into(),
            nodes: vec![WorkflowNode {
                id: "node_1".into(),
                r#type: NodeType::GateHitl,
                name: "Validation Agent".into(),
                params: json!({}),
            }],
            edges: vec![],
        };

        scheduler.register_workflow(def);
        let mut instance = WorkflowInstance::new("wf_test_hitl", HashMap::new());

        let _ = scheduler
            .run_step(&mut instance)
            .await
            .expect("Run step failed");

        assert_eq!(instance.status, ExecutionStatus::Paused);

        scheduler
            .resume_node(&mut instance, "node_1", true)
            .await
            .expect("Resume failed");

        assert_eq!(
            instance.node_states.get("node_1"),
            Some(&ExecutionStatus::Completed)
        );
        assert_eq!(instance.status, ExecutionStatus::Running);
    }

    #[tokio::test]
    #[ignore = "Nécessite AiOrchestrator (Llama.cpp/Qdrant)"]
    async fn test_decision_branching() {
        let mut scheduler = setup_test().await;

        let def = WorkflowDefinition {
            id: "wf_decision".into(),
            entry: "node_start".into(),
            nodes: vec![
                WorkflowNode {
                    id: "node_start".into(),
                    r#type: NodeType::Task,
                    name: "Analyze Input".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "node_a".into(),
                    r#type: NodeType::End,
                    name: "Chemin A (Approved)".into(),
                    params: json!({}),
                },
                WorkflowNode {
                    id: "node_b".into(),
                    r#type: NodeType::End,
                    name: "Chemin B (Rejected)".into(),
                    params: json!({}),
                },
            ],
            edges: vec![
                WorkflowEdge {
                    from: "node_start".into(),
                    to: "node_a".into(),
                    condition: Some("validation == 'approved'".into()),
                },
                WorkflowEdge {
                    from: "node_start".into(),
                    to: "node_b".into(),
                    condition: Some("validation == 'rejected'".into()),
                },
            ],
        };
        scheduler.register_workflow(def);

        let mut ctx_a = HashMap::new();
        ctx_a.insert("validation".to_string(), json!("approved"));
        let mut instance = WorkflowInstance::new("wf_decision", ctx_a);

        instance
            .node_states
            .insert("node_start".into(), ExecutionStatus::Completed);

        scheduler.run_step(&mut instance).await.unwrap();

        assert!(
            instance.node_states.contains_key("node_a"),
            "Le noeud A aurait dû être déclenché (Approved)"
        );
    }
}
