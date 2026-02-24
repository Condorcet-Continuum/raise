use super::{HandlerContext, NodeHandler};
use crate::utils::{prelude::*, HashMap};
use crate::workflow_engine::{ExecutionStatus, NodeType, WorkflowNode};
use async_trait::async_trait;

pub struct GateHitlHandler;

#[async_trait]
impl NodeHandler for GateHitlHandler {
    fn node_type(&self) -> NodeType {
        NodeType::GateHitl
    }

    async fn execute(
        &self,
        node: &WorkflowNode,
        _context: &mut HashMap<String, Value>,
        _shared_ctx: &HandlerContext<'_>,
    ) -> RaiseResult<ExecutionStatus> {
        tracing::warn!(
            "⏸️ Workflow en pause (Validation Humaine Requise) : '{}'",
            node.name
        );
        Ok(ExecutionStatus::Paused)
    }
}
