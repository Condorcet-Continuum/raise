use super::{HandlerContext, NodeHandler};
use crate::utils::prelude::*;
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};

pub struct GateHitlHandler;

#[async_interface]
impl NodeHandler for GateHitlHandler {
    fn node_type(&self) -> NodeType {
        NodeType::GateHitl
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        _context: &mut UnorderedMap<String, JsonValue>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        tracing::warn!(
            "⏸️ Workflow en pause (Validation Humaine Requise) : '{}'",
            node.name
        );
        Ok(ExecutionStatus::Paused)
    }
}
