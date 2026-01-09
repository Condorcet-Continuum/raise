// FICHIER : src-tauri/src/workflow_engine/scheduler.rs

use super::{
    executor::WorkflowExecutor, state_machine::WorkflowStateMachine, ExecutionStatus,
    WorkflowDefinition, WorkflowInstance,
};
use crate::ai::orchestrator::AiOrchestrator;
use crate::utils::Result;
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
// Import des outils MCP
use super::tools::{AgentTool, SystemMonitorTool};

/// Le Scheduler pilote l'exécution des workflows et assure le pont avec l'IA.
pub struct WorkflowScheduler {
    pub executor: WorkflowExecutor, // Public pour accès dans les tests si besoin
    pub definitions: HashMap<String, WorkflowDefinition>,
    pub orchestrator: Arc<Mutex<AiOrchestrator>>,
}

impl WorkflowScheduler {
    pub fn new(orchestrator: Arc<Mutex<AiOrchestrator>>) -> Self {
        // On initialise l'executor et on y injecte les outils par défaut
        let mut executor = WorkflowExecutor::new(orchestrator.clone());

        // Enregistrement de l'outil système (Veto Démo)
        executor.register_tool(Box::new(SystemMonitorTool));

        Self {
            executor,
            definitions: HashMap::new(),
            orchestrator,
        }
    }

    // Méthode pour permettre l'ajout dynamique d'outils
    pub fn register_tool(&mut self, tool: Box<dyn AgentTool>) {
        self.executor.register_tool(tool);
    }

    pub fn register_workflow(&mut self, def: WorkflowDefinition) {
        self.definitions.insert(def.id.clone(), def);
    }

    /// Exécute une étape du workflow (trouve les nœuds éligibles et les lance)
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
                // Appel à l'executeur (IA, Décision, etc.)
                let status = self
                    .executor
                    .execute_node(node, &json!(instance.context))
                    .await?;

                // Transition d'état dans la machine à états
                // CORRECTION ICI : On convertit l'erreur &str en AppError
                sm.transition(instance, &node_id, status)
                    .map_err(|e| crate::utils::AppError::from(e.to_string()))?;

                // Si pause demandée (HITL), on arrête la boucle immédiate
                if status == ExecutionStatus::Paused {
                    instance.status = ExecutionStatus::Paused;
                    return Ok(false);
                }

                progress_made = true;
            }
        }

        Ok(progress_made)
    }

    /// Reprend l'exécution après une validation humaine (HITL).
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

        // On repasse l'instance en Running pour que le prochain run_step fonctionne
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
    use crate::model_engine::types::ProjectModel;
    use crate::workflow_engine::{
        ExecutionStatus, NodeType, WorkflowDefinition, WorkflowEdge, WorkflowNode,
    };
    use serde_json::json;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    // Helper de test
    async fn setup_test() -> WorkflowScheduler {
        let model = ProjectModel::default();
        let orch = AiOrchestrator::new(model, "http://127.0.0.1:6334", "http://127.0.0.1:8081")
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "Impossible de créer l'orchestrateur (Vérifiez que llama.cpp/Qdrant tournent)"
                )
            });

        let shared_orch = Arc::new(Mutex::new(orch));
        WorkflowScheduler::new(shared_orch)
    }

    #[tokio::test]
    #[ignore = "Nécessite AiOrchestrator (Llama.cpp/Qdrant)"]
    async fn test_full_agentic_loop() {
        let mut scheduler = setup_test().await;

        // Définition d'un workflow simple : Start -> Gate -> End
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
            .resume_node(&mut instance, "node_1", true) // Approved
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

        // Graphe : Start -> (Decision) -> A ou B
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
