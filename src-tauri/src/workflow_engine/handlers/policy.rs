// FICHIER : src-tauri/src/workflow_engine/handlers/policy.rs
use crate::utils::prelude::*;

use super::{HandlerContext, NodeHandler};
use crate::rules_engine::ast::Expr;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct GatePolicyHandler;

#[async_interface]
impl NodeHandler for GatePolicyHandler {
    fn node_type(&self) -> NodeType {
        NodeType::QualityGate
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        let rule_name = node
            .params
            .get("rule")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");

        // 🎯 FIX : Macro d'observabilité RAISE
        user_info!("WF_POLICY_CHECK_START", json_value!({ "rule": rule_name }));

        let ast_val = match node.params.get("ast") {
            Some(ast) => ast,
            None => {
                // 🎯 FIX : Macro d'observabilité RAISE
                user_warn!(
                    "WF_POLICY_NO_AST",
                    json_value!({ "rule": rule_name, "action": "Fail-Safe Block" })
                );
                return Ok(ExecutionStatus::Failed);
            }
        };

        let expr: Expr = match json::deserialize_from_value(ast_val.clone()) {
            Ok(e) => e,
            Err(e) => {
                // 🎯 FIX : Macro d'observabilité RAISE
                user_error!(
                    "WF_POLICY_MALFORMED_AST",
                    json_value!({ "rule": rule_name, "error": e.to_string() })
                );
                return Ok(ExecutionStatus::Failed);
            }
        };

        let context_value = json::serialize_to_value(&*context).unwrap_or(json_value!({}));
        let provider = NoOpDataProvider;

        match Evaluator::evaluate(&expr, &context_value, &provider).await {
            Ok(res_cow) => {
                let is_triggered = match res_cow.as_ref() {
                    JsonValue::Bool(b) => *b,
                    _ => false,
                };

                if is_triggered {
                    // 🎯 FIX : Macro d'observabilité RAISE
                    user_error!(
                        "WF_POLICY_VETO_TRIGGERED",
                        json_value!({ "rule": rule_name })
                    );
                    Ok(ExecutionStatus::Failed)
                } else {
                    // 🎯 FIX : Macro d'observabilité RAISE
                    user_success!("WF_POLICY_PASSED", json_value!({ "rule": rule_name }));
                    Ok(ExecutionStatus::Completed)
                }
            }
            Err(e) => {
                // 🎯 FIX : Macro d'observabilité RAISE
                user_error!(
                    "WF_POLICY_EVAL_ERROR",
                    json_value!({ "rule": rule_name, "error": e.to_string() })
                );
                Ok(ExecutionStatus::Failed)
            }
        }
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
    async fn test_policy_handler_valid_ast_pass() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };
        let handler = GatePolicyHandler;

        let ast = json_value!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v1".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: VIBRATION".into(),
            params: json_value!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        let mut data_ctx = UnorderedMap::from([("sensor_vibration".into(), json_value!(2.5))]);
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_valid_ast_trigger() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };
        let handler = GatePolicyHandler;

        let ast = json_value!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v2".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: VIBRATION".into(),
            params: json_value!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        let mut data_ctx = UnorderedMap::from([("sensor_vibration".into(), json_value!(12.0))]);
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_without_ast() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };
        let handler = GatePolicyHandler;

        let node = WorkflowNode {
            id: "v3".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: NO_AST".into(),
            params: json_value!({ "rule": "MISSING_RULES" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_with_malformed_ast() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };
        let handler = GatePolicyHandler;

        let node = WorkflowNode {
            id: "v4".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: BROKEN_AST".into(),
            params: json_value!({ "rule": "BROKEN", "ast": "Ceci n'est pas un JSON valide" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }
}
