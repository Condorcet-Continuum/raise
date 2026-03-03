// FICHIER : src-tauri/src/workflow_engine/handlers/mcp.rs
use super::{HandlerContext, NodeHandler};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct McpHandler;

#[async_trait]
impl NodeHandler for McpHandler {
    fn node_type(&self) -> NodeType {
        NodeType::CallMcp
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        // Extraction sécurisée du nom de l'outil MCP
        let tool_name = match node.params.get("tool_name") {
            Some(val) => match val.as_str() {
                Some(s) => s,
                None => raise_error!(
                    "ERR_MCP_INVALID_PARAM",
                    context = json!({
                        "node_id": node.id,
                        "param": "tool_name",
                        "expected": "string",
                        "received": val,
                        "hint": "Le nom de l'outil MCP doit être une chaîne de caractères (ex: 'fetch_url')."
                    })
                ),
            },
            None => raise_error!(
                "ERR_MCP_MISSING_PARAM",
                context = json!({
                    "node_id": node.id,
                    "param": "tool_name",
                    "action": "CallMcp",
                    "hint": "Le paramètre 'tool_name' est obligatoire pour les nœuds de type CallMcp."
                })
            ),
        };

        let default_args = json!({});
        let args = node.params.get("arguments").unwrap_or(&default_args);
        let output_key = node
            .params
            .get("output_key")
            .and_then(|v| v.as_str())
            .unwrap_or("tool_output");

        tracing::info!("🛠️ Appel Outil MCP : {} avec {:?}", tool_name, args);

        if let Some(tool) = shared_ctx.tools.get(tool_name) {
            match tool.execute(args).await {
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
    use crate::model_engine::types::ProjectModel;
    use crate::plugins::manager::PluginManager;
    use crate::utils::{Arc, AsyncMutex};
    use crate::workflow_engine::critic::WorkflowCritic;
    use crate::workflow_engine::tools::{AgentTool, SystemMonitorTool};

    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::config::test_mocks::{inject_mock_component, AgentDbSandbox};
    use crate::utils::data::json;

    // 🎯 FIX : La fonction prend la DB et la config en paramètres
    async fn setup_dummy_context_with_tool(
        storage: Arc<crate::json_db::storage::StorageEngine>,
        config: &crate::utils::config::AppConfig,
    ) -> (
        Arc<AsyncMutex<AiOrchestrator>>,
        Arc<PluginManager>,
        WorkflowCritic,
        HashMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
    ) {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 1. 🎯 INJECTION DES MOCKS : L'orchestrateur IA trouve ses petits
        inject_mock_component(
            &manager,
            "llm",
            json!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json!({ "provider": "mock" })).await;

        // 2. 🎯 INITIALISATION : On utilise le StorageEngine de la Sandbox
        let orch = AiOrchestrator::new(ProjectModel::default(), Some(storage.clone()))
            .await
            .unwrap();

        let plugin_manager = Arc::new(PluginManager::new(&storage, None));
        let critic = WorkflowCritic::default();

        let mut tools: HashMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>> =
            HashMap::new();
        let monitor = SystemMonitorTool;
        tools.insert(monitor.name().to_string(), Box::new(monitor));

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
    async fn test_mcp_handler_success_and_injection() {
        // 1. 🎯 MAGIE : La Sandbox initialise le dossier isolé et le schéma
        let sandbox = AgentDbSandbox::new().await;

        // 2. Injection dans le faux contexte
        let (orch, pm, critic, tools) =
            setup_dummy_context_with_tool(sandbox.db.clone(), &sandbox.config).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = McpHandler;

        let node = WorkflowNode {
            id: "tool_1".into(),
            r#type: NodeType::CallMcp,
            name: "Lire Capteur CPU".into(),
            params: json!({
                "tool_name": "read_system_metrics",
                "arguments": { "sensor_id": "cpu_core" },
                "output_key": "my_cpu_result"
            }),
        };

        let mut data_ctx = HashMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Completed);
        assert!(
            data_ctx.contains_key("my_cpu_result"),
            "Le résultat de l'outil doit être injecté sous la clé demandée"
        );
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_mcp_handler_missing_tool_fails_safely() {
        let sandbox = AgentDbSandbox::new().await;
        let (orch, pm, critic, tools) =
            setup_dummy_context_with_tool(sandbox.db.clone(), &sandbox.config).await;

        let ctx = HandlerContext {
            orchestrator: &orch,
            plugin_manager: &pm,
            critic: &critic,
            tools: &tools,
        };
        let handler = McpHandler;

        let node = WorkflowNode {
            id: "tool_2".into(),
            r#type: NodeType::CallMcp,
            name: "Outil Inconnu".into(),
            params: json!({ "tool_name": "outil_magique_qui_n_existe_pas" }),
        };

        let mut data_ctx = HashMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        // L'outil n'existe pas, l'exécution doit échouer proprement
        assert_eq!(result, ExecutionStatus::Failed);
    }
}
