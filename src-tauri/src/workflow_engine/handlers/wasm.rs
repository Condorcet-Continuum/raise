// FICHIER : src-tauri/src/workflow_engine/handlers/wasm.rs
use super::{HandlerContext, NodeHandler};
use crate::utils::prelude::*;
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct WasmHandler;

#[async_interface]
impl NodeHandler for WasmHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Wasm
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        let plugin_id = node
            .params
            .get("plugin_id")
            .and_then(|v| v.as_str())
            .unwrap_or(&node.id);

        tracing::info!("🔮 [WASM Hub] Appel du plugin : {}", plugin_id);

        let mandate_ctx = context.get("_mandate").cloned();

        match shared_ctx
            .plugin_manager
            .run_plugin_with_context(plugin_id, mandate_ctx)
            .await
        {
            Ok((exit_code, signals)) => {
                for signal in signals {
                    tracing::info!("📡 [SIGNAL PLUGIN] {} : {:?}", plugin_id, signal);
                    context.insert(format!("{}_signal", plugin_id), signal);
                }

                if exit_code == 1 {
                    Ok(ExecutionStatus::Completed)
                } else {
                    tracing::warn!(
                        "⛔ [WASM VETO] Plugin a retourné un échec (Code {})",
                        exit_code
                    );
                    Ok(ExecutionStatus::Failed)
                }
            }
            Err(e) => {
                tracing::error!("❌ [WASM ERROR] Échec exécution : {}", e);
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

        (
            SharedRef::new(AsyncMutex::new(orch)),
            plugin_manager,
            WorkflowCritic::default(),
            UnorderedMap::new(),
            manager,
        )
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_wasm_handler_missing_plugin_fails_safely() {
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
        let handler = WasmHandler;

        let node = WorkflowNode {
            id: "wasm_1".into(),
            r#type: NodeType::Wasm,
            name: "Test Plugin".into(),
            params: json_value!({ "plugin_id": "plugin_inconnu" }),
        };

        let mut data_ctx = UnorderedMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        assert_eq!(result, ExecutionStatus::Failed);
    }
}
