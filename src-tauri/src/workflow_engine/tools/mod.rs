// FICHIER : src-tauri/src/workflow_engine/tools/mod.rs

use crate::utils::prelude::*;
// 🎯 NOUVEAU : Import du contexte
use super::handlers::HandlerContext;

/// Définition d'un Outil que l'Agent (ou le Workflow) peut appeler.
#[async_interface]
pub trait AgentTool: Send + Sync + FmtDebug {
    /// Nom unique de l'outil (ex: "read_system_metrics", "fs_write")
    fn name(&self) -> &str;

    /// Description pour le LLM (Quand utiliser cet outil ?)
    fn description(&self) -> &str;

    /// Schéma JSON des arguments attendus
    fn parameters_schema(&self) -> JsonValue;

    /// 🎯 NOUVEAU : L'exécution reçoit maintenant le contexte (donc la base de données !)
    async fn execute(
        &self,
        args: &JsonValue,
        context: &HandlerContext<'_>,
    ) -> RaiseResult<JsonValue>;
}

pub mod system_tools;
pub use system_tools::SystemMonitorTool;

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct MockEchoTool;

    #[async_interface]
    impl AgentTool for MockEchoTool {
        fn name(&self) -> &str {
            "mock_echo"
        }
        fn description(&self) -> &str {
            "Renvoie l'argument 'input'"
        }
        fn parameters_schema(&self) -> JsonValue {
            json_value!({})
        }

        async fn execute(
            &self,
            args: &JsonValue,
            _context: &HandlerContext<'_>,
        ) -> RaiseResult<JsonValue> {
            let input = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json_value!({ "echo": input }))
        }
    }

    #[async_test]
    async fn test_tool_polymorphism() {
        let tool: Box<dyn AgentTool> = Box::new(MockEchoTool);
        assert_eq!(tool.name(), "mock_echo");
    }
}
