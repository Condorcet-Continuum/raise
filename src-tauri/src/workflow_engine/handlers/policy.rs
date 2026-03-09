use crate::utils::prelude::*;

use super::{HandlerContext, NodeHandler};
use crate::rules_engine::ast::Expr;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct GatePolicyHandler;

#[async_interface]
impl NodeHandler for GatePolicyHandler {
    fn node_type(&self) -> NodeType {
        NodeType::GatePolicy
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
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

        let expr: Expr = match json::deserialize_from_value(ast_val.clone()) {
            Ok(e) => e,
            Err(e) => {
                tracing::error!("❌ AST malformé pour '{}' : {}. Blocage.", rule_name, e);
                return Ok(ExecutionStatus::Failed);
            }
        };

        let context_value = serde_json::to_value(&*context).unwrap_or(json_value!({}));
        let provider = NoOpDataProvider;

        match Evaluator::evaluate(&expr, &context_value, &provider).await {
            Ok(res_cow) => {
                let is_triggered = match res_cow.as_ref() {
                    JsonValue::Bool(b) => *b,
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
// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::orchestrator::AiOrchestrator;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::workflow_engine::critic::WorkflowCritic;

    // 🎯 IMPORTS AJOUTÉS : On récupère notre Sandbox et les injecteurs
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    /// Helper pour générer rapidement un HandlerContext factice sans surcharger les tests
    // 🎯 FIX : La fonction prend la DB et la config en paramètres
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

        // 1. 🎯 INJECTION DES MOCKS : L'orchestrateur IA trouve sa configuration
        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        // 2. 🎯 INITIALISATION : On utilise le StorageEngine de la Sandbox
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(storage.clone()))
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
    async fn test_policy_handler_valid_ast_pass() {
        // 1. 🎯 MAGIE : La Sandbox initialise le dossier isolé
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
        let handler = GatePolicyHandler;

        // Règle : Bloquer SI sensor_vibration > 8.0
        let ast = json_value!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v1".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: VIBRATION".into(),
            params: json_value!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        // Cas A : La valeur est sûre (2.5 n'est pas > 8.0) -> Veto NON déclenché (Completed)
        let mut data_ctx = UnorderedMap::from([("sensor_vibration".into(), json_value!(2.5))]);
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_valid_ast_trigger() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = GatePolicyHandler;

        // Règle : Bloquer SI sensor_vibration > 8.0
        let ast = json_value!({ "gt": [{"var": "sensor_vibration"}, {"val": 8.0}] });
        let node = WorkflowNode {
            id: "v2".into(),
            r#type: NodeType::GatePolicy,
            name: "VETO: VIBRATION".into(),
            params: json_value!({ "rule": "VIBRATION_MAX", "ast": ast }),
        };

        // Cas B : La valeur est dangereuse (12.0 > 8.0) -> Veto DÉCLENCHÉ (Failed)
        let mut data_ctx = UnorderedMap::from([("sensor_vibration".into(), json_value!(12.0))]);
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_without_ast() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config).await;

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
            params: json_value!({ "rule": "MISSING_RULES" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        // Sécurité maximale : Pas de règle = on bloque le flux
        assert_eq!(result, ExecutionStatus::Failed);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_fails_safe_with_malformed_ast() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools) =
            setup_dummy_context(sandbox.db.clone(), &sandbox.config).await;

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
            params: json_value!({ "rule": "BROKEN", "ast": "Ceci n'est pas un JSON valide" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        // L'erreur de parsing ne doit pas faire crasher l'application, mais bloquer l'exécution
        assert_eq!(result, ExecutionStatus::Failed);
    }
}
