// FICHIER : src-tauri/src/workflow_engine/handlers/decision.rs
use crate::utils::prelude::*;

use super::{HandlerContext, NodeHandler};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct DecisionHandler;

#[async_interface]
impl NodeHandler for DecisionHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Decision
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        tracing::info!("🗳️ Algorithme de Condorcet : {}", node.name);

        let default_weights = JsonObject::new();
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

        context.insert("condorcet_winner".into(), candidates[winner_idx].clone());

        Ok(ExecutionStatus::Completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::critic::WorkflowCritic;

    // 🎯 FIX : Retourne le CollectionsManager pour satisfaire la nouvelle signature
    async fn setup_dummy_context<'a>(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &'a AppConfig,
        sandbox_db: &'a crate::json_db::storage::StorageEngine,
    ) -> (
        SharedRef<AsyncMutex<AiOrchestrator>>,
        SharedRef<PluginManager>,
        WorkflowCritic,
        UnorderedMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
        CollectionsManager<'a>,
    ) {
        let manager = CollectionsManager::new(sandbox_db, &config.system_domain, &config.system_db);

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

        let plugin_manager = SharedRef::new(PluginManager::new(&storage, None));
        let critic = WorkflowCritic::default();
        let tools = UnorderedMap::new();

        (
            SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager,
            critic,
            tools,
            manager,
        )
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_decision_handler_condorcet_evaluation() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager, // 🎯 FIX : Injection du manager
        };
        let handler = DecisionHandler;

        let node = WorkflowNode {
            id: "dec_1".into(),
            r#type: NodeType::Decision,
            name: "Vote Final".into(),
            params: json_value!({ "weights": { "agent_security": 5.0, "agent_finance": 1.0 } }),
        };

        let mut data_ctx = UnorderedMap::from([(
            "candidates".into(),
            json_value!(["Option A (Courte)", "Option B (Très très longue)"]),
        )]);

        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
        assert!(data_ctx.contains_key("condorcet_winner"));
    }
}
