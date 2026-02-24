// FICHIER : src-tauri/src/workflow_engine/executor.rs

use crate::utils::{prelude::*, Arc, AsyncMutex, HashMap};

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
    pub orchestrator: Arc<AsyncMutex<AiOrchestrator>>,
    pub plugin_manager: Arc<PluginManager>,
    critic: WorkflowCritic,
    tools: HashMap<String, Box<dyn AgentTool>>,
    handlers: HashMap<NodeType, Box<dyn NodeHandler>>,
}

impl WorkflowExecutor {
    /// Crée un nouvel exécuteur lié à l'intelligence centrale et au Hub de Plugins
    pub fn new(
        orchestrator: Arc<AsyncMutex<AiOrchestrator>>,
        plugin_manager: Arc<PluginManager>,
    ) -> Self {
        let mut handlers: HashMap<NodeType, Box<dyn NodeHandler>> = HashMap::new();

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
            tools: HashMap::new(),
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
        context: &mut HashMap<String, Value>,
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
// TESTS UNITAIRES (CONSERVÉS POUR ASSURER LA RÉTROCOMPATIBILITÉ GLOBALE)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::ProjectModel;
    use crate::utils::{config::test_mocks, data::json, io::tempdir};
    use crate::workflow_engine::tools::SystemMonitorTool;

    use crate::json_db::schema::registry::SchemaRegistry;
    use crate::json_db::schema::SchemaValidator;
    use crate::json_db::test_utils::{ensure_db_exists, init_test_env};

    async fn create_test_executor_with_tools() -> WorkflowExecutor {
        test_mocks::inject_mock_config();
        let model = ProjectModel::default();
        let orch = AiOrchestrator::new(model, None).await.unwrap();
        let dir = tempdir().unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(dir.path().to_path_buf()));
        let plugin_manager = Arc::new(PluginManager::new(&storage, None));

        let mut exec = WorkflowExecutor::new(Arc::new(AsyncMutex::new(orch)), plugin_manager);
        exec.register_tool(Box::new(SystemMonitorTool));
        exec
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_gate_pause() {
        let executor = create_test_executor_with_tools().await;
        let node = WorkflowNode {
            id: "node_pause".into(),
            r#type: NodeType::GateHitl,
            name: "Human Check".into(),
            params: Value::Null,
        };
        let mut ctx = HashMap::new();
        let result = executor.execute_node(&node, &mut ctx).await;
        assert_eq!(result.unwrap(), ExecutionStatus::Paused);
    }

    #[tokio::test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_bridge_loading_and_compilation() {
        let env = init_test_env().await;
        test_mocks::inject_mock_config();

        let cfg = &env.cfg;
        let space = &env.space;
        let db = &env.db;
        ensure_db_exists(cfg, space, db).await;

        let dest_schemas = cfg.db_schemas_root(space, db).join("v1");
        let _ = std::fs::create_dir_all(&dest_schemas);

        let mandate_schema = json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "meta": { "type": "object", "properties": { "author": { "type": "string" }, "version": { "type": "string" } }, "required": ["author", "version"] },
                "governance": { "type": "object" },
                "hardLogic": { "type": "object" }
            },
            "required": ["id", "meta", "governance"]
        });
        std::fs::write(
            dest_schemas.join("mandates.json"),
            mandate_schema.to_string(),
        )
        .unwrap();

        let reg = SchemaRegistry::from_db(cfg, space, db).await.unwrap();
        let root_uri = reg.uri("mandates.json");
        let _validator = SchemaValidator::compile_with_registry(&root_uri, &reg).unwrap();

        let manager = CollectionsManager::new(&env.storage, space, db);
        manager
            .create_collection("mandates", Some("mandates.json".to_string()))
            .await
            .unwrap();

        let valid_mandate = json!({
            "id": "mandate_prod",
            "meta": { "author": "BridgeTest", "version": "1.0", "status": "ACTIVE" },
            "governance": { "strategy": "SAFETY_FIRST", "condorcetWeights": { "agent_security": 1.0 } },
            "hardLogic": { "vetos": [{ "rule": "VIBRATION_MAX", "active": true, "action": "STOP" }] },
            "observability": { "heartbeatMs": 100 }
        });

        manager
            .insert_raw("mandates", &valid_mandate)
            .await
            .unwrap();
        let result = WorkflowExecutor::load_and_prepare_workflow(&manager, "mandate_prod").await;

        assert!(result.is_ok());
        let workflow = result.unwrap();
        assert!(workflow.nodes.len() >= 4);
    }
}
