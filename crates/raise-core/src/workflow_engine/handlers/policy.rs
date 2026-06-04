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
    use crate::utils::testing::{AgentDbSandbox, DbSandbox}; // 🎯 Ajout de DbSandbox
    use crate::workflow_engine::critic::WorkflowCritic;

    /// 🎯 HELPER ZÉRO DETTE : Injecte les autorisations et configurations requises
    /// pour permettre à l'Orchestrateur de s'initialiser dans les tests du workflow.
    async fn inject_ai_mocks(manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let config = AppConfig::get();
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        let session_schema_uri = format!(
            "db://{}/{}/schemas/v2/agents/memory/chat_session.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        let _ = DbSandbox::mock_db(manager).await;

        let _ = manager
            .create_collection("components", &generic_schema)
            .await;
        let _ = manager
            .create_collection("service_configs", &generic_schema)
            .await;

        manager
            .upsert_document(
                "components",
                json_value!({ "_id": "ref:components:handle:rag", "handle": "rag" }),
            )
            .await?;
        manager.upsert_document("service_configs", json_value!({ "_id": "mock_rag", "component_id": "ref:components:handle:rag", "service_settings": { "collection_name": "raise_knowledge_base" } })).await?;

        manager.upsert_document("components", json_value!({ "_id": "ref:components:handle:ai_world_model", "handle": "ai_world_model" })).await?;
        manager.upsert_document("service_configs", json_value!({ "_id": "mock_wm", "component_id": "ref:components:handle:ai_world_model", "service_settings": { "vocab_size": 1000, "active": true } })).await?;

        manager.upsert_document("components", json_value!({ "_id": "ref:components:handle:ai_memory_store", "handle": "ai_memory_store" })).await?;
        manager.upsert_document("service_configs", json_value!({ "_id": "mock_mem", "component_id": "ref:components:handle:ai_memory_store", "service_settings": { "max_history_tokens": 4096, "collection_name": "raise_conversation_history", "schema_uri": session_schema_uri, "active": true } })).await?;

        Ok(())
    }

    async fn setup_policy_test_context<'a>(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &'a AppConfig,
        sandbox_db: &'a crate::json_db::storage::StorageEngine,
    ) -> RaiseResult<(
        SharedRef<AsyncMutex<AiOrchestrator>>,
        SharedRef<PluginManager>,
        WorkflowCritic,
        UnorderedMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
        CollectionsManager<'a>,
    )> {
        // 🎯 RÉSILIENCE MOUNT POINTS : Utilisation dynamique de la config système
        let manager = CollectionsManager::new(
            sandbox_db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 🎯 FIX CRITIQUE : Préparation du terrain pour l'IA
        inject_ai_mocks(&manager).await?;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, storage.clone(), None)
            .await
            .expect("Orchestrator setup failed");

        Ok((
            SharedRef::new(AsyncMutex::new(orch)),
            SharedRef::new(PluginManager::new(&storage, None)),
            WorkflowCritic::default(),
            UnorderedMap::new(),
            manager,
        ))
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_policy_handler_valid_ast_pass() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await?;

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
        let sandbox = AgentDbSandbox::new().await?;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await?;

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
        let sandbox = AgentDbSandbox::new().await?;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await?;

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
        let sandbox = AgentDbSandbox::new().await?;
        let (orch, pm, critic, tools, manager) =
            setup_policy_test_context(sandbox.db.clone(), &sandbox.config, &sandbox.db).await?;

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
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        // Le validateur doit s'appuyer sur les partitions système SSOT
        assert!(!config.mount_points.system.domain.is_empty());
        assert!(!config.mount_points.system.db.is_empty());
        Ok(())
    }
}
