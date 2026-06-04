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
    use crate::utils::testing::{AgentDbSandbox, DbSandbox}; // 🎯 Ajout de DbSandbox
    use crate::workflow_engine::critic::WorkflowCritic;
    use crate::workflow_engine::tools::{AgentTool, SystemMonitorTool};

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

        // 🎯 FIX CRITIQUE : Préparation du terrain pour l'IA
        inject_ai_mocks(&manager).await?;

        let orch = AiOrchestrator::new(ProjectModel::default(), &manager, storage.clone(), None)
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
