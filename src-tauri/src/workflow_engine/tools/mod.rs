// FICHIER : src-tauri/src/workflow_engine/tools/mod.rs

use crate::utils::Result;
use serde_json::Value;
use std::fmt::Debug;

/// Définition d'un Outil que l'Agent (ou le Workflow) peut appeler.
/// Inspiré par le standard MCP (Model Context Protocol).
/// Contrairement aux Agents, ces outils doivent être DÉTERMINISTES (ou physiques).
#[async_trait::async_trait]
pub trait AgentTool: Send + Sync + Debug {
    /// Nom unique de l'outil (ex: "read_system_metrics", "fs_write")
    fn name(&self) -> &str;

    /// Description pour le LLM (Quand utiliser cet outil ?)
    fn description(&self) -> &str;

    /// Schéma JSON des arguments attendus
    fn parameters_schema(&self) -> Value;

    /// Exécution de l'outil avec des arguments JSON
    async fn execute(&self, args: &Value) -> Result<Value>;
}

// Module pour les implémentations concrètes
pub mod system_tools;

// Re-export pour faciliter l'usage
pub use system_tools::SystemMonitorTool;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- MOCK TOOL POUR TESTER LE TRAIT ---
    #[derive(Debug)]
    struct MockEchoTool;

    #[async_trait::async_trait]
    impl AgentTool for MockEchoTool {
        fn name(&self) -> &str {
            "mock_echo"
        }
        fn description(&self) -> &str {
            "Renvoie l'argument 'input'"
        }
        fn parameters_schema(&self) -> Value {
            json!({})
        }

        async fn execute(&self, args: &Value) -> Result<Value> {
            let input = args.get("input").and_then(|v| v.as_str()).unwrap_or("");
            Ok(json!({ "echo": input }))
        }
    }

    #[tokio::test]
    async fn test_tool_polymorphism() {
        // Teste si on peut stocker et utiliser l'outil via son Trait (Box<dyn AgentTool>)
        let tool: Box<dyn AgentTool> = Box::new(MockEchoTool);

        assert_eq!(tool.name(), "mock_echo");

        let args = json!({ "input": "Hello Raise" });
        let result = tool.execute(&args).await.expect("Execution failed");

        assert_eq!(result["echo"], "Hello Raise");
    }
}
