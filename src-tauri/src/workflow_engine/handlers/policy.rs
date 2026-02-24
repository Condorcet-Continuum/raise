use super::{HandlerContext, NodeHandler};
use crate::rules_engine::ast::Expr;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct GatePolicyHandler;

#[async_trait]
impl NodeHandler for GatePolicyHandler {
    fn node_type(&self) -> NodeType {
        NodeType::GatePolicy
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
        _shared_ctx: &HandlerContext<'_>, // Pas besoin d'outils externes pour évaluer l'AST
    ) -> RaiseResult<ExecutionStatus> {
        let rule_name = node
            .params
            .get("rule")
            .and_then(|v| v.as_str())
            .unwrap_or("UNKNOWN");

        tracing::info!("🛡️ [Handler] Vérification Veto : {}", rule_name);

        let ast_val = match node.params.get("ast") {
            Some(ast) => ast,
            None => {
                tracing::warn!(
                    "⚠️ Aucune règle AST pour le Veto '{}'. Blocage (Fail-Safe).",
                    rule_name
                );
                return Ok(ExecutionStatus::Failed);
            }
        };

        let expr: Expr = match serde_json::from_value(ast_val.clone()) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("❌ AST malformé pour '{}' : {}. Blocage.", rule_name, e);
                return Ok(ExecutionStatus::Failed);
            }
        };

        let context_value = serde_json::to_value(&*context).unwrap_or(json!({}));
        let provider = NoOpDataProvider;

        match Evaluator::evaluate(&expr, &context_value, &provider).await {
            Ok(res_cow) => {
                let is_triggered = match res_cow.as_ref() {
                    Value::Bool(b) => *b,
                    _ => false,
                };

                if is_triggered {
                    tracing::error!("🚨 VETO DYNAMIQUE DÉCLENCHÉ : {}", rule_name);
                    Ok(ExecutionStatus::Failed)
                } else {
                    tracing::info!("✅ Veto non déclenché : {}", rule_name);
                    Ok(ExecutionStatus::Completed)
                }
            }
            Err(e) => {
                tracing::error!("❌ Erreur d'évaluation (Fail-Safe) : {}", e);
                Ok(ExecutionStatus::Failed)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::{config::test_mocks, io::tempdir, Arc, AsyncMutex};
    use crate::workflow_engine::critic::WorkflowCritic;

    /// Helper pour générer rapidement un HandlerContext factice sans surcharger les tests
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
    async fn test_policy_handler_valid_ast_pass() {
        let (orch, pm, critic, tools) = setup_dummy_context().await;
        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = GatePolicyHandler;

        // Règle : Bloquer SI sensor_vibration > 8.0
        let ast = json!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v1".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: VIBRATION".into(),
            params: json!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        // Cas A : La valeur est sûre (2.5 n'est pas > 8.0) -> Veto NON déclenché (Completed)
        let mut data_ctx = HashMap::from([("sensor_vibration".into(), json!(2.5))]);
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_valid_ast_trigger() {
        let (orch, pm, critic, tools) = setup_dummy_context().await;
        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = GatePolicyHandler;

        // Règle : Bloquer SI sensor_vibration > 8.0
        let ast = json!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v2".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: VIBRATION".into(),
            params: json!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        // Cas B : La valeur est dangereuse (12.0 > 8.0) -> Veto DÉCLENCHÉ (Failed)
        let mut data_ctx = HashMap::from([("sensor_vibration".into(), json!(12.0))]);
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_without_ast() {
        let (orch, pm, critic, tools) = setup_dummy_context().await;
        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = GatePolicyHandler;

        // On omet le champ "ast" intentionnellement
        let node = WorkflowNode {
            id: "v3".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: NO_AST".into(),
            params: json!({ "rule": "MISSING_RULES" }),
        };

        let mut data_ctx = HashMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        // Sécurité maximale : Pas de règle = on bloque le flux
        assert_eq!(result, ExecutionStatus::Failed);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_with_malformed_ast() {
        let (orch, pm, critic, tools) = setup_dummy_context().await;
        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = GatePolicyHandler;

        // On met un AST qui n'est pas compréhensible par le rules_engine
        let node = WorkflowNode {
            id: "v4".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: BROKEN_AST".into(),
            params: json!({ "rule": "BROKEN", "ast": "Ceci n'est pas un JSON valide" }),
        };

        let mut data_ctx = HashMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        // L'erreur de parsing ne doit pas faire crasher l'application, mais bloquer l'exécution
        assert_eq!(result, ExecutionStatus::Failed);
    }
}
