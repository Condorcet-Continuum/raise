// FICHIER : src-tauri/src/workflow_engine/handlers/mod.rs

pub mod decision;
pub mod end;
pub mod hitl;
pub mod mcp;
pub mod policy;
pub mod task;
pub mod wasm;
pub mod world_model;

use crate::ai::orchestrator::AiOrchestrator;
use crate::json_db::collections::manager::CollectionsManager;
use crate::plugins::manager::PluginManager;
use crate::utils::prelude::*;

use super::critic::WorkflowCritic;
use super::tools::AgentTool;
use super::{ExecutionStatus, NodeType, WorkflowNode};

/// Le Contexte Partagé : La "boîte à outils" que l'Exécuteur prête aux Handlers
pub struct HandlerContext<'a> {
    pub orchestrator: &'a SharedRef<AsyncMutex<AiOrchestrator>>,
    pub plugin_manager: &'a SharedRef<PluginManager>,
    pub critic: &'a WorkflowCritic,
    pub tools: &'a UnorderedMap<String, Box<dyn AgentTool>>,
    pub manager: &'a CollectionsManager<'a>,
}

/// Le Contrat : Chaque stratégie d'exécution doit implémenter ceci
#[async_interface]
pub trait NodeHandler: Send + Sync {
    /// Indique quel type de nœud ce handler sait traiter
    fn node_type(&self) -> NodeType;

    /// Exécute la logique métier du nœud
    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut UnorderedMap<String, JsonValue>,
        shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus>;
}
