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
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::critic::WorkflowCritic;

    // 🎯 FIX : On passe la config en plus pour utiliser le CollectionsManager
    async fn setup_dummy_context(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &AppConfig,
    ) -> (
        SharedRef<AsyncMutex<AiOrchestrator>>,
        SharedRef<PluginManager>,
        WorkflowCritic,
        UnorderedMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
    ) {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 1. 🎯 INJECTION DES MOCKS : L'orchestrateur ne paniquera plus !
        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        // 2. 🎯 ATTENTION : On passe Some(storage.clone()) au lieu de None
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
        )
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_decision_handler_condorcet_evaluation() {
        // 1. Initialisation de la sandbox
        let sandbox = AgentDbSandbox::new().await;

        // 2. On passe la base et la config à notre setup
        let (orch, pm, critic, tools) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config).await;

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
            params: json_value!({ "weights": { "agent_security": 5.0, "agent_finance": 1.0 } }),
        };

        let mut data_ctx = UnorderedMap::from([(
            "candidates".into(),
            json_value!(["Option A (Courte)", "Option B (Très très longue)"]),
        )]);

        // 3. Exécution du Handler
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
        assert!(
            data_ctx.contains_key("condorcet_winner"),
            "Le vainqueur doit être injecté au contexte"
        );
    }
}
