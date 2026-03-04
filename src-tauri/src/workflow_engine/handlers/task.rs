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
    ) -> RaiseResult<ExecutionStatus> {
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
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::{Arc, AsyncMutex};
    use crate::workflow_engine::critic::WorkflowCritic;

    // 🎯 IMPORTS AJOUTÉS : On récupère notre Sandbox et les injecteurs
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::data::json;
    use crate::utils::mock::{inject_mock_component, AgentDbSandbox};

    // 🎯 FIX : La fonction prend la DB et la config de la Sandbox en paramètres
    async fn setup_dummy_context(
        storage: Arc<crate::json_db::storage::StorageEngine>,
        config: &crate::utils::config::AppConfig,
    ) -> (
        Arc<AsyncMutex<AiOrchestrator>>,
        Arc<PluginManager>,
        WorkflowCritic,
        HashMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
    ) {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 1. 🎯 INJECTION DES MOCKS : L'orchestrateur IA est configuré de façon transparente
        inject_mock_component(
            &manager,
            "llm",
            json!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json!({ "provider": "mock" })).await;

        // 2. 🎯 INITIALISATION : On utilise le StorageEngine de la Sandbox (important : Some(storage.clone()))
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(storage.clone()))
            .await
            .unwrap();

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
        // 1. 🎯 MAGIE : La Sandbox initialise le dossier isolé et le schéma
        let sandbox = AgentDbSandbox::new().await;

        // 2. Injection dans le faux contexte
        let (orch, pm, critic, tools) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config).await;

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

        // 3. Exécution de la tâche
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
    }
}
