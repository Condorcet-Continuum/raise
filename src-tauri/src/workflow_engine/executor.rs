// FICHIER : src-tauri/src/workflow_engine/executor.rs

use crate::utils::prelude::*;

use super::compiler::WorkflowCompiler;
use super::handlers::{
    decision::DecisionHandler, end::EndHandler, hitl::GateHitlHandler, mcp::McpHandler,
    policy::GatePolicyHandler, task::TaskHandler, wasm::WasmHandler, HandlerContext, NodeHandler,
};
use super::mandate::Mandate;
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
    /// Crée un nouvel exécuteur lié à l'intelligence centrale et au Hub de Plugins
    pub fn new(
        orchestrator: SharedRef<AsyncMutex<AiOrchestrator>>,
        plugin_manager: SharedRef<PluginManager>,
    ) -> Self {
        let mut handlers: UnorderedMap<NodeType, Box<dyn NodeHandler>> = UnorderedMap::new();

        // RECUTEMENT DE TOUS LES OUVRIERS SPÉCIALISÉS !
        handlers.insert(NodeType::GatePolicy, Box::new(GatePolicyHandler));
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

    /// Permet au Scheduler d'injecter des outils dynamiquement
    pub fn register_tool(&mut self, tool: Box<dyn AgentTool>) {
        self.tools.insert(tool.name().to_string(), tool);
    }

    // ========================================================================
    // LE PONT : Chargement et Compilation Sécurisés
    // ========================================================================

    pub async fn load_and_prepare_workflow(
        manager: &CollectionsManager<'_>,
        mandate_id: &str,
    ) -> RaiseResult<WorkflowDefinition> {
        let mandate = Mandate::fetch_from_store(manager, mandate_id).await?;

        tracing::info!(
            "📜 Mandat chargé et validé : {} v{} (Stratégie: {:?})",
            mandate.meta.author,
            mandate.meta.version,
            mandate.governance.strategy
        );

        let workflow = WorkflowCompiler::compile(&mandate);

        tracing::info!(
            "🏗️ Workflow compilé avec succès : {} ({}) - {} noeuds",
            workflow.id,
            mandate.id,
            workflow.nodes.len()
        );

        Ok(workflow)
    }

    // ========================================================================
    // EXECUTION DES NOEUDS (ROUTAGE)
    // ========================================================================

    pub async fn execute_node(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
    ) -> RaiseResult<ExecutionStatus> {
        tracing::info!("⚙️ Exécution : {} ({:?})", node.name, node.r#type);

        let shared_ctx = HandlerContext {
            orchestrator: &self.orchestrator,
            plugin_manager: &self.plugin_manager,
            critic: &self.critic,
            tools: &self.tools,
        };

        // Routage dynamique unique. Plus de match !
        if let Some(handler) = self.handlers.get(&node.r#type) {
            handler.execute(node, context, &shared_ctx).await
        } else {
            tracing::error!(
                "❌ Erreur Critique : Aucun Handler défini pour le type de nœud {:?}",
                node.r#type
            );
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
    use crate::model_engine::types::ProjectModel;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use crate::workflow_engine::tools::SystemMonitorTool;
    // N'oubliez pas d'importer le CollectionsManager si ce n'est pas déjà fait
    use crate::json_db::collections::manager::CollectionsManager;

    // 🎯 FIX : On passe la Config en plus pour pouvoir initialiser le Manager
    async fn create_test_executor_with_tools(
        storage: SharedRef<crate::json_db::storage::StorageEngine>,
        config: &AppConfig,
    ) -> WorkflowExecutor {
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        // 1. 🎯 INJECTION DES MOCKS : On nourrit l'orchestrateur avec des composants factices
        inject_mock_component(
            &manager,
            "llm",
            json_value!({ "provider": "mock", "model": "test" }),
        )
        .await;
        inject_mock_component(&manager, "rag", json_value!({ "provider": "mock" })).await;

        let model = ProjectModel::default();

        // 2. 🎯 ATTENTION : On passe `Some(storage.clone())` à l'orchestrateur
        // pour qu'il utilise bien la DB de la Sandbox et non une DB globale !
        let orch = AiOrchestrator::new(model, Some(storage.clone()))
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

        // 🎯 FIX : On passe db et config
        let executor = create_test_executor_with_tools(sandbox.db.clone(), &sandbox.config).await;

        let node = WorkflowNode {
            id: "node_pause".into(),
            r#type: NodeType::GateHitl,
            name: "Human Check".into(),
            params: JsonValue::Null,
        };

        let mut ctx = UnorderedMap::new();
        let result = executor.execute_node(&node, &mut ctx).await;
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

        let valid_mandate = json_value!({
            "_id": "mandate_prod",
            "meta": { "author": "BridgeTest", "version": "1.0", "status": "ACTIVE" },
            "governance": { "strategy": "SAFETY_FIRST", "condorcetWeights": { "agent_security": 1.0 } },
            "hardLogic": { "vetos": [{ "rule": "VIBRATION_MAX", "active": true, "action": "STOP" }] },
            "observability": { "heartbeatMs": 100 }
        });

        manager
            .create_collection(
                "mandates",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .insert_raw("mandates", &valid_mandate)
            .await
            .expect("L'insertion du mandat a échoué");

        let result = WorkflowExecutor::load_and_prepare_workflow(&manager, "mandate_prod").await;

        assert!(result.is_ok(), "Le chargement du workflow a échoué");
        let workflow = result.unwrap();

        assert!(workflow.nodes.len() >= 4);
    }
}
