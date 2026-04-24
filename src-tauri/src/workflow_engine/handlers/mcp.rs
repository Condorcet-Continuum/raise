// FICHIER : src-tauri/src/workflow_engine/handlers/mcp.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE
use crate::workflow_engine::handlers::{HandlerContext, NodeHandler};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct McpHandler;

#[async_interface]
impl NodeHandler for McpHandler {
    fn node_type(&self) -> NodeType {
        NodeType::CallMcp
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        user_info!("INF_MCP_START", json_value!({ "node": node.name }));

        // 1. Extraction sécurisée des paramètres via Match
        let tool_name = match node.params.get("tool_name").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => raise_error!(
                "ERR_MCP_MISSING_TOOL_NAME",
                context = json_value!({ "node_id": node.id, "param": "tool_name" })
            ),
        };

        let arguments = match node.params.get("arguments") {
            Some(args) => args.clone(),
            None => json_value!({}),
        };

        let output_key = match node.params.get("output_key").and_then(|v| v.as_str()) {
            Some(s) => s.to_string(),
            None => "tool_output".to_string(),
        };

        // 2. Vérification de la disponibilité dans le registre d'outils
        let tool = match shared_ctx.tools.get(&tool_name) {
            Some(t) => t,
            None => {
                user_error!("ERR_MCP_TOOL_NOT_FOUND", json_value!({ "tool": tool_name }));
                return Ok(ExecutionStatus::Failed);
            }
        };

        // 3. Exécution de l'outil avec gestion de la résilience
        user_info!("INF_MCP_INVOKING", json_value!({ "tool": tool_name }));

        match tool.execute(&arguments, shared_ctx).await {
            Ok(output) => {
                // Nettoyage du résultat (on extrait 'value' si c'est un objet enveloppé)
                let cleaned_output = match output.as_object() {
                    Some(obj) => obj.get("value").cloned().unwrap_or(output),
                    None => output,
                };

                context.insert(output_key.clone(), cleaned_output);

                user_success!(
                    "SUC_MCP_COMPLETED",
                    json_value!({ "node": node.id, "output_key": output_key })
                );
                Ok(ExecutionStatus::Completed)
            }
            Err(e) => {
                user_error!(
                    "ERR_MCP_TOOL_EXECUTION",
                    json_value!({ "tool": tool_name, "error": e.to_string() })
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
    use crate::workflow_engine::tools::{AgentTool, SystemMonitorTool};

    async fn setup_mcp_test_context<'a>(
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

        inject_mock_component(&manager, "llm", json_value!({ "provider": "mock" })).await?;

        inject_mock_component(
            &manager,
            "ai_graph_store",
            json_value!({
                "embedding_dim": 16,
                "provider": "native"
            }),
        )
        .await?;

        inject_mock_component(
            &manager,
            "ai_world_model",
            json_value!({
                "vocab_size": 16,
                "embedding_dim": 16,
                "action_dim": 8,
                "hidden_dim": 32,
                "use_gpu": false
            }),
        )
        .await?;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, storage.clone())
            .await
            .expect("Orchestrator setup failed");

        let _plugin_manager = SharedRef::new(PluginManager::new(&storage, None));

        let mut tools: UnorderedMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>> =
            UnorderedMap::new();
        let monitor = SystemMonitorTool;
        tools.insert(monitor.name().to_string(), Box::new(monitor));

        Ok((
            SharedRef::new(AsyncMutex::new(orch)),
            SharedRef::new(PluginManager::new(&storage, None)),
            WorkflowCritic::default(),
            tools,
            manager,
        ))
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_mcp_handler_success_and_injection() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let (orch, pm, critic, tools, manager) =
            setup_mcp_test_context(sandbox.db.clone(), &config, &sandbox.db).await?;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let node = WorkflowNode {
            id: "tool_1".into(),
            r#type: NodeType::CallMcp,
            name: "Lire CPU".into(),
            params: json_value!({
                "tool_name": "read_system_metrics",
                "arguments": { "sensor_id": "cpu_core" },
                "output_key": "my_cpu_result"
            }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = McpHandler.execute(&node, &mut data_ctx, &ctx).await?;

        assert_eq!(result, ExecutionStatus::Completed);
        assert!(data_ctx.contains_key("my_cpu_result"));
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_mcp_handler_missing_tool_fails_safely() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let (orch, pm, critic, tools, manager) =
            setup_mcp_test_context(sandbox.db.clone(), &config, &sandbox.db).await?;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let node = WorkflowNode {
            id: "tool_2".into(),
            r#type: NodeType::CallMcp,
            name: "Outil Fantôme".into(),
            params: json_value!({ "tool_name": "ghost_tool" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = McpHandler.execute(&node, &mut data_ctx, &ctx).await?;

        assert_eq!(result, ExecutionStatus::Failed);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à l'absence de paramètres obligatoires
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_resilience_missing_params_match() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let (orch, pm, critic, tools, manager) =
            setup_mcp_test_context(sandbox.db.clone(), &config, &sandbox.db).await?;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager,
        };

        let node = WorkflowNode {
            id: "tool_err".into(),
            r#type: NodeType::CallMcp,
            name: "Fail Node".into(),
            params: json_value!({}), // Manque tool_name
        };

        let mut data_ctx = UnorderedMap::new();
        let result = McpHandler.execute(&node, &mut data_ctx, &ctx).await;

        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_MCP_MISSING_TOOL_NAME");
                Ok(())
            }
            _ => panic!("Le handler aurait dû lever ERR_MCP_MISSING_TOOL_NAME"),
        }
    }

    /// 🎯 NOUVEAU TEST : Validation de la partition système via Mount Points
    #[async_test]
    async fn test_mcp_mount_point_resolution() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        // On vérifie que le handler peut s'appuyer sur les partitions système SSOT
        assert!(!config.mount_points.system.domain.is_empty());
        assert!(!config.mount_points.system.db.is_empty());
        Ok(())
    }
}
