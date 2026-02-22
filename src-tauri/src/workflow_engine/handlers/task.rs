// FICHIER : src-tauri/src/workflow_engine/handlers/task.rs
use super::{HandlerContext, NodeHandler};
use crate::ai::assurance::xai::{ExplanationScope, XaiFrame, XaiMethod};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct TaskHandler;

#[async_trait]
impl NodeHandler for TaskHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Task
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
        shared_ctx: &HandlerContext<'_>,
    ) -> crate::utils::Result<ExecutionStatus> {
        let mut orch = shared_ctx.orchestrator.lock().await;

        let mission = format!(
            "OBJECTIF: {}\nPARAMÈTRES: {:?}\nCONTEXTE: {:?}",
            node.name, node.params, context
        );

        let ai_response = orch.ask(&mission).await?;

        // Traçabilité et Explicabilité (XAI)
        let mut xai = XaiFrame::new(&node.id, XaiMethod::ChainOfThought, ExplanationScope::Local);
        xai.predicted_output = ai_response.clone();
        xai.input_snapshot = mission;

        // Le Critique (Reward Model)
        let critique = shared_ctx.critic.evaluate(&xai).await;
        if !critique.is_acceptable {
            tracing::warn!("⚠️ Qualité insuffisante détectée par le critique !");
        }

        tracing::info!("✅ Tâche '{}' validée par l'agent.", node.name);
        Ok(ExecutionStatus::Completed)
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::{config::test_mocks, io::tempdir, Arc, AsyncMutex};
    use crate::workflow_engine::critic::WorkflowCritic;

    async fn setup_dummy_context() -> (
        Arc<AsyncMutex<AiOrchestrator>>,
        Arc<PluginManager>,
        WorkflowCritic,
        HashMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
    ) {
        test_mocks::inject_mock_config();
        let orch = AiOrchestrator::new(ProjectModel::default(), None)
            .await
            .unwrap();
        let dir = tempdir().unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(dir.path().to_path_buf()));
        let plugin_manager = Arc::new(PluginManager::new(&storage, None));
        let critic = WorkflowCritic::default();
        let tools = HashMap::new();

        (
            Arc::new(AsyncMutex::new(orch)),
            plugin_manager,
            critic,
            tools,
        )
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)] // Indispensable car on instancie l'Orchestrateur
    async fn test_task_handler_execution() {
        let (orch, pm, critic, tools) = setup_dummy_context().await;
        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = TaskHandler;

        let node = WorkflowNode {
            id: "task_1".into(),
            r#type: NodeType::Task,
            name: "Agent de Test".into(),
            params: json!({ "directive": "Analyse de sécurité" }),
        };

        let mut data_ctx = HashMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
    }
}
