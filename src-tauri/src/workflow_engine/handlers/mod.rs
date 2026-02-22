pub mod decision;
pub mod end;
pub mod hitl;
pub mod mcp;
pub mod policy;
pub mod task;
pub mod wasm;

use crate::ai::orchestrator::AiOrchestrator;
use crate::plugins::manager::PluginManager;
use crate::utils::{prelude::*, Arc, AsyncMutex, HashMap};

use super::critic::WorkflowCritic;
use super::tools::AgentTool;
use super::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

/// Le Contexte Partagé : La "boîte à outils" que l'Exécuteur prête aux Handlers
pub struct HandlerContext<'a> {
    pub orchestrator: &'a Arc<AsyncMutex<AiOrchestrator>>,
    pub plugin_manager: &'a Arc<PluginManager>,
    pub critic: &'a WorkflowCritic,
    pub tools: &'a HashMap<String, Box<dyn AgentTool>>,
}

/// Le Contrat : Chaque stratégie d'exécution doit implémenter ceci
#[async_trait]
pub trait NodeHandler: Send + Sync {
    /// Indique quel type de nœud ce handler sait traiter
    fn node_type(&self) -> NodeType;

    /// Exécute la logique métier du nœud
    async fn execute(
        &self,
        node: &WorkflowNode,
        context: &mut HashMap<String, Value>,
        shared_ctx: &HandlerContext<'_>,
    ) -> crate::utils::Result<ExecutionStatus>;
}
