use super::{HandlerContext, NodeHandler};
use crate::utils::prelude::*;
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct EndHandler;

#[async_interface]
impl NodeHandler for EndHandler {
    fn node_type(&self) -> NodeType {
        NodeType::End
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        _context: &mut UnorderedMap<String, JsonValue>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        tracing::info!("🏁 Nœud de fin atteint : '{}'", node.name);
        Ok(ExecutionStatus::Completed)
    }
}
