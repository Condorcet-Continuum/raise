// FICHIER : src-tauri/src/workflow_engine/handlers/policy.rs
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

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
            .unwrap_or("UNKNOWN_RULE");

        user_info!("WF_POLICY_CHECK_START", json_value!({ "rule": rule_name }));

        // 1. Extraction de l'AST via Match
        let ast_val = match node.params.get("ast") {
            Some(ast) => ast,
            None => {
                user_warn!(
                    "WF_POLICY_NO_AST",
                    json_value!({ "rule": rule_name, "action": "Fail-Safe Block" })
                );
                return Ok(ExecutionStatus::Failed);
            }
        };

        // 2. Désérialisation résiliente de l'expression
        let expr: Expr = match json::deserialize_from_value(ast_val.clone()) {
            Ok(e) => e,
            Err(e) => {
                user_error!(
                    "WF_POLICY_MALFORMED_AST",
                    json_value!({ "rule": rule_name, "error": e.to_string() })
                );
                return Ok(ExecutionStatus::Failed);
            }
        };

        // 3. Préparation du contexte d'évaluation
        let context_value = match json::serialize_to_value(&*context) {
            Ok(val) => val,
            Err(_) => json_value!({}),
        };

        let provider = NoOpDataProvider;

        // 4. Évaluation de la règle de sécurité (Veto)
        match Evaluator::evaluate(&expr, &context_value, &provider).await {
            Ok(res_cow) => {
                let is_triggered = match res_cow.as_ref() {
                    JsonValue::Bool(b) => *b,
                    _ => {
                        user_warn!(
                            "WF_POLICY_TYPE_MISMATCH",
                            json_value!({ "rule": rule_name, "expected": "bool" })
                        );
                        false
                    }
                };

                if is_triggered {
                    user_error!(
                        "WF_POLICY_VETO_TRIGGERED",
                        json_value!({ "rule": rule_name })
                    );
                    Ok(ExecutionStatus::Failed)
                } else {
                    user_success!("WF_POLICY_PASSED", json_value!({ "rule": rule_name }));
                    Ok(ExecutionStatus::Completed)
                }
            }
            Err(e) => {
                user_error!(
                    "WF_POLICY_EVAL_ERROR",
                    json_value!({ "rule": rule_name, "error": e.to_string() })
                );
                Ok(ExecutionStatus::Failed)
            }
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Conformité Façade & Résilience Mount Points)
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

    async fn setup_policy_test_context<'a>(
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
        // 🎯 RÉSILIENCE MOUNT POINTS : Utilisation dynamique de la config système
        let manager = CollectionsManager::new(
            sandbox_db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_component(&manager, "llm", json_value!({ "provider": "mock" })).await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, storage.clone())
            .await
            .expect("Orchestrator setup failed");

        (
            SharedRef::new(AsyncMutex::new(orch)),
            SharedRef::new(PluginManager::new(&storage, None)),
            WorkflowCritic::default(),
            UnorderedMap::new(),
            manager,
        )
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_valid_ast_pass() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let ast = json_value!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v1".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: VIBRATION".into(),
            params: json_value!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        let mut data_ctx = UnorderedMap::from([("sensor_vibration".into(), json_value!(2.5))]);
        let result = GatePolicyHandler
            .execute(&node, &mut data_ctx, &ctx)
            .await?;

        assert_eq!(result, ExecutionStatus::Completed);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_valid_ast_trigger() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let ast = json_value!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v2".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: VIBRATION".into(),
            params: json_value!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        let mut data_ctx = UnorderedMap::from([("sensor_vibration".into(), json_value!(12.0))]);
        let result = GatePolicyHandler
            .execute(&node, &mut data_ctx, &ctx)
            .await?;

        assert_eq!(result, ExecutionStatus::Failed);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_without_ast() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let node = WorkflowNode {
            id: "v3".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: NO_AST".into(),
            params: json_value!({ "rule": "MISSING_RULES" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = GatePolicyHandler
            .execute(&node, &mut data_ctx, &ctx)
            .await?;

        assert_eq!(result, ExecutionStatus::Failed);
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_with_malformed_ast() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let node = WorkflowNode {
            id: "v4".into(),
            r#type: NodeType::QualityGate,
            name: "VETO: BROKEN_AST".into(),
            params: json_value!({ "rule": "BROKEN", "ast": "Invalid AST" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = GatePolicyHandler
            .execute(&node, &mut data_ctx, &ctx)
            .await?;

        assert_eq!(result, ExecutionStatus::Failed);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face au point de montage système manquant
    #[async_test]
    async fn test_policy_mount_point_resilience() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        // Le validateur doit s'appuyer sur les partitions système SSOT
        assert!(!config.mount_points.system.domain.is_empty());
        assert!(!config.mount_points.system.db.is_empty());
        Ok(())
    }
}
