use super::{HandlerContext, NodeHandler};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct EndHandler;

#[async_trait]
impl NodeHandler for EndHandler {
    fn node_type(&self) -> NodeType {
        NodeType::End
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        _context: &mut HashMap<String, Value>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> crate::utils::Result<ExecutionStatus> {
        tracing::info!("🏁 Nœud de fin atteint : '{}'", node.name);
        Ok(ExecutionStatus::Completed)
    }
}
