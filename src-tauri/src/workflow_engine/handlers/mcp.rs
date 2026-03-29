// FICHIER : src-tauri/src/workflow_engine/handlers/mcp.rs
use super::{HandlerContext, NodeHandler};
use crate::utils::prelude::*;
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
        let tool_name = match node.params.get("tool_name") {
            Some(val) => match val.as_str() {
                Some(s) => s,
                None => raise_error!(
                    "ERR_MCP_INVALID_PARAM",
                    context = json_value!({ "node_id": node.id, "param": "tool_name", "expected": "string" })
                ),
            },
            None => raise_error!(
                "ERR_MCP_MISSING_PARAM",
                context =
                    json_value!({ "node_id": node.id, "param": "tool_name", "action": "CallMcp" })
            ),
        };

        let default_args = json_value!({});
        let args = node.params.get("arguments").unwrap_or(&default_args);
        let output_key = node
            .params
            .get("output_key")
            .and_then(|v| v.as_str())
            .unwrap_or("tool_output");

        tracing::info!("🛠️ Appel Outil MCP : {} avec {:?}", tool_name, args);

        if let Some(tool) = shared_ctx.tools.get(tool_name) {
            // 🎯 NOUVEAU : On passe le shared_ctx à l'outil
            match tool.execute(args, shared_ctx).await {
                Ok(output) => {
                    tracing::info!("✅ Résultat Outil injecté dans '{}'", output_key);
                    let cleaned_output = if let Some(obj) = output.as_object() {
                        obj.get("value").cloned().unwrap_or(output)
                    } else {
                        output
                    };
                    context.insert(output_key.to_string(), cleaned_output);
                    Ok(ExecutionStatus::Completed)
                }
                Err(e) => {
                    tracing::error!("❌ Erreur outil : {}", e);
                    Ok(ExecutionStatus::Failed)
                }
            }
        } else {
            tracing::error!("❌ Outil introuvable : {}", tool_name);
            Ok(ExecutionStatus::Failed)
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
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::critic::WorkflowCritic;
    use crate::workflow_engine::tools::{AgentTool, SystemMonitorTool};

    // 🎯 FIX : On retourne aussi le manager pour qu'il survive à la portée
    async fn setup_dummy_context_with_tool<'a>(
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

        let mut tools: UnorderedMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>> =
            UnorderedMap::new();
        let monitor = SystemMonitorTool;
        tools.insert(monitor.name().to_string(), Box::new(monitor));

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
    async fn test_mcp_handler_success_and_injection() {
        let sandbox = AgentDbSandbox::new().await;

        // Extraction des éléments
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context_with_tool(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager, // 🎯 Ajout
        };
        let handler = McpHandler;

        let node = WorkflowNode {
            id: "tool_1".into(),
            r#type: NodeType::CallMcp,
            name: "Lire Capteur CPU".into(),
            params: json_value!({
                "tool_name": "read_system_metrics",
                "arguments": { "sensor_id": "cpu_core" },
                "output_key": "my_cpu_result"
            }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
        assert!(data_ctx.contains_key("my_cpu_result"));
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_mcp_handler_missing_tool_fails_safely() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools, manager) =
            setup_dummy_context_with_tool(sandbox.db.clone(), &sandbox.config, &sandbox.db).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
            manager: &manager, // 🎯 Ajout
        };
        let handler = McpHandler;

        let node = WorkflowNode {
            id: "tool_2".into(),
            r#type: NodeType::CallMcp,
            name: "Outil Inconnu".into(),
            params: json_value!({ "tool_name": "outil_magique_qui_n_existe_pas" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }
}
