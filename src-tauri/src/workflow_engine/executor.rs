// FICHIER : src-tauri/src/workflow_engine/executor.rs

use crate::utils::prelude::*;

use super::compiler::WorkflowCompiler;
use super::handlers::{
    decision::DecisionHandler, end::EndHandler, hitl::GateHitlHandler, mcp::McpHandler,
    policy::GatePolicyHandler, task::TaskHandler, wasm::WasmHandler, HandlerContext, NodeHandler,
};
use super::tools::AgentTool;
use super::{critic::WorkflowCritic, ExecutionStatus, NodeType, WorkflowDefinition, WorkflowNode};
use crate::plugins::manager::PluginManager;

use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::collections::manager::CollectionsManager;

/// L'Exécuteur est le routeur principal. Il délègue la logique aux Handlers spécialisés.
pub struct WorkflowExecutor {
    pub orchestrator: SharedRef<AsyncMutex<AiOrchestrator>>,
    pub plugin_manager: SharedRef<PluginManager>,
    critic: WorkflowCritic,
    tools: UnorderedMap<String, Box<dyn AgentTool>>,
    handlers: UnorderedMap<NodeType, Box<dyn NodeHandler>>,
}

impl WorkflowExecutor {
    pub fn new(
        orchestrator: SharedRef<AsyncMutex<AiOrchestrator>>,
        plugin_manager: SharedRef<PluginManager>,
    ) -> Self {
        let mut handlers: UnorderedMap<NodeType, Box<dyn NodeHandler>> = UnorderedMap::new();

        // 🎯 FIX : Utilisation du nom QualityGate (aligné sur MBSE)
        handlers.insert(NodeType::QualityGate, Box::new(GatePolicyHandler));
        handlers.insert(NodeType::Task, Box::new(TaskHandler));
        handlers.insert(NodeType::Decision, Box::new(DecisionHandler));
        handlers.insert(NodeType::CallMcp, Box::new(McpHandler));
        handlers.insert(NodeType::Wasm, Box::new(WasmHandler));
        handlers.insert(NodeType::GateHitl, Box::new(GateHitlHandler));
        handlers.insert(NodeType::End, Box::new(EndHandler));

        Self {
            orchestrator,
            plugin_manager,
            critic: WorkflowCritic::default(),
            tools: UnorderedMap::new(),
            handlers,
        }
    }

    pub fn register_tool(&mut self, tool: Box<dyn AgentTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    // ========================================================================
    // LE PONT : Chargement et Compilation Sécurisés
    // ========================================================================

    pub async fn load_and_prepare_workflow(
        manager: &CollectionsManager<'_>,
        mission_handle: &str,
    ) -> RaiseResult<WorkflowDefinition> {
        tracing::info!(
            "📥 Début de compilation pour la mission : {}",
            mission_handle
        );

        // Compilation asynchrone (tissage Mission/Mandat/Template)
        let workflow = WorkflowCompiler::compile(manager, mission_handle).await?;

        tracing::info!(
            "🏗️ Workflow compilé : {} ({} noeuds)",
            workflow.handle,
            workflow.nodes.len()
        );

        Ok(workflow)
    }

    // ========================================================================
    // EXECUTION DES NOEUDS (ROUTAGE)
    // ========================================================================

    pub async fn execute_node<'a>(
        &'a self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        manager: &'a CollectionsManager<'a>,
    ) -> RaiseResult<ExecutionStatus> {
        tracing::info!("⚙️ Exécution : {} ({:?})", node.name, node.r#type);

        let shared_ctx = HandlerContext {
            orchestrator: &self.orchestrator,
            plugin_manager: &self.plugin_manager,
            critic: &self.critic,
            tools: &self.tools,
            manager,
        };

        if let Some(handler) = self.handlers.get(&node.r#type) {
            handler.execute(node, context, &shared_ctx).await
        } else {
            raise_error!(
                "ERR_WF_HANDLER_NOT_FOUND",
                context = json_value!({
                    "node_id": node.id,
                    "node_type": format!("{:?}", node.r#type),
                    "hint": "Aucun exécuteur (Handler) associé à ce type de nœud. Vérifiez l'initialisation du WorkflowExecutor."
                })
            )
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::ProjectModel;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::tools::SystemMonitorTool;

    async fn create_test_executor_with_tools(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &AppConfig,
    ) -> WorkflowExecutor {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);
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

        let mut exec = WorkflowExecutor::new(SharedRef::new(AsyncMutex::new(orch)), plugin_manager);
        exec.register_tool(Box::new(SystemMonitorTool));
        exec
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_gate_pause() {
        let sandbox = AgentDbSandbox::new().await;
        let executor = create_test_executor_with_tools(sandbox.db.clone(), &sandbox.config).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let node = WorkflowNode {
            id: "node_pause".into(),
            r#type: NodeType::GateHitl,
            name: "Human Check".into(),
            params: JsonValue::Null,
        };

        let mut ctx = UnorderedMap::new();
        let result = executor.execute_node(&node, &mut ctx, &manager).await;
        assert_eq!(result.unwrap(), ExecutionStatus::Paused);
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_bridge_loading_and_compilation() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // Mocks JSON alignés sur les schémas stricts
        manager
            .create_collection(
                "workflow_definitions",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document(
                "workflow_definitions",
                json_value!({
                    "handle": "tpl_1",
                    "name": "Template Test",
                    "entry": "start",
                    "nodes": [{"id": "start", "type": "task", "name": "Start", "params": {}}],
                    "edges": []
                }),
            )
            .await
            .unwrap();

        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .upsert_document(
                "mandates",
                json_value!({
                    "handle": "mandate-1",
                    "name": "Mandat de Test",
                    // 🎯 FIX : Utilisation de mandator_id avec un UUID valide au lieu de "author"
                    "meta": { "mandator_id": "00000000-0000-0000-0000-000000000000", "version": "1.0", "status": "ACTIVE" },
                    "governance": { "strategy": "SAFETY_FIRST", "condorcetWeights": {} },
                    "hardLogic": { "vetos": [] },
                    "observability": { "heartbeatMs": 100 }
                }),
            )
            .await
            .unwrap();

        manager
            .create_collection(
                "missions",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .upsert_document(
                "missions",
                json_value!({
                    "handle": "mission-prod",
                    "name": "Mission de Production",
                    "mandate_id": "mandate-1",
                    "squad_id": "squad_1",
                    "workflow_template_id": "tpl_1",
                    "status": "draft"
                }),
            )
            .await
            .unwrap();

        let result = WorkflowExecutor::load_and_prepare_workflow(&manager, "mission-prod").await;
        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert!(workflow.handle.starts_with("wf_compiled_mandate-1"));
    }
}
