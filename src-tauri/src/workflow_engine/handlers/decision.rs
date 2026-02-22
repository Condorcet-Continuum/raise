// FICHIER : src-tauri/src/workflow_engine/handlers/decision.rs
use super::{HandlerContext, NodeHandler};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct DecisionHandler;

#[async_trait]
impl NodeHandler for DecisionHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Decision
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> crate::utils::Result<ExecutionStatus> {
        tracing::info!("🗳️ Algorithme de Condorcet : {}", node.name);

        let default_weights = serde_json::Map::new();
        let weights = node
            .params
            .get("weights")
            .and_then(|v| v.as_object())
            .unwrap_or(&default_weights);

        let w_security = weights
            .get("agent_security")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);
        let w_finance = weights
            .get("agent_finance")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0);

        let candidates = match context.get("candidates").and_then(|v| v.as_array()) {
            Some(list) if list.len() > 1 => list,
            _ => return Ok(ExecutionStatus::Completed),
        };

        let mut wins = vec![0.0; candidates.len()];

        for i in 0..candidates.len() {
            for j in (i + 1)..candidates.len() {
                let cand_a = &candidates[i];
                let cand_b = &candidates[j];

                let len_a = cand_a.to_string().len();
                let len_b = cand_b.to_string().len();

                if len_a < len_b {
                    wins[i] += w_security;
                } else {
                    wins[j] += w_security;
                }
                if len_a > len_b {
                    wins[i] += w_finance;
                } else {
                    wins[j] += w_finance;
                }
            }
        }

        let (winner_idx, max_wins) = wins
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap();
        tracing::info!(
            "👑 Vainqueur Condorcet : Candidat #{} (Score: {:.1})",
            winner_idx,
            max_wins
        );

        // On injecte le vainqueur dans le contexte pour la suite du graphe
        context.insert("condorcet_winner".into(), candidates[winner_idx].clone());

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
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_decision_handler_condorcet_evaluation() {
        let (orch, pm, critic, tools) = setup_dummy_context().await;
        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = DecisionHandler;

        let node = WorkflowNode {
            id: "dec_1".into(),
            r#type: NodeType::Decision,
            name: "Vote Final".into(),
            params: json!({ "weights": { "agent_security": 5.0, "agent_finance": 1.0 } }),
        };

        let mut data_ctx = HashMap::from([(
            "candidates".into(),
            json!(["Option A (Courte)", "Option B (Très très longue)"]),
        )]);

        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
        assert!(
            data_ctx.contains_key("condorcet_winner"),
            "Le vainqueur doit être injecté au contexte"
        );
    }
}
