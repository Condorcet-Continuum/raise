// FICHIER : src-tauri/src/workflow_engine/handlers/wasm.rs
use super::{HandlerContext, NodeHandler};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct WasmHandler;

#[async_trait]
impl NodeHandler for WasmHandler {
    fn node_type(&self) -> NodeType {
        NodeType::Wasm
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
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

    // 🎯 IMPORTS AJOUTÉS : On récupère notre Sandbox et les injecteurs
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::config::test_mocks::{inject_mock_component, AgentDbSandbox};
    use crate::utils::data::json;

    // 🎯 FIX : La fonction prend la DB et la config de la Sandbox en paramètres
    async fn setup_dummy_context(
        storage: Arc<crate::json_db::storage::StorageEngine>,
        config: &crate::utils::config::AppConfig,
    ) -> (
        Arc<AsyncMutex<AiOrchestrator>>,
        Arc<PluginManager>,
        WorkflowCritic,
        HashMap<String, Box<dyn crate::workflow_engine::tools::AgentTool>>,
    ) {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 1. 🎯 INJECTION DES MOCKS : Configuration de l'orchestrateur IA
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

        (
            Arc::new(AsyncMutex::new(orch)),
            plugin_manager,
            WorkflowCritic::default(),
            HashMap::new(),
        )
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_wasm_handler_missing_plugin_fails_safely() {
        // 1. 🎯 MAGIE : La Sandbox initialise le dossier isolé et le schéma
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
        let handler = WasmHandler;

        let node = WorkflowNode {
            id: "wasm_1".into(),
            r#type: NodeType::Wasm,
            name: "Test Plugin".into(),
            params: json!({ "plugin_id": "plugin_inconnu" }),
        };

        let mut data_ctx = HashMap::new();
        let result = handler.execute(&node, &mut data_ctx, &ctx).await.unwrap();

        // Un plugin manquant doit retourner Failed
        assert_eq!(result, ExecutionStatus::Failed);
    }
}
