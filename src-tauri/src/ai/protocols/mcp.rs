// FICHIER : src-tauri/src/ai/protocols/mcp.rs

use crate::utils::{async_trait, prelude::*, Arc, DateTime, HashMap, Utc, Uuid};
// =========================================================================
// 1. STRUCTURES DE DONNÉES (Payloads) - Celles que vous avez fournies
// =========================================================================

/// Représente une demande d'exécution d'outil (Function Call).
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCall {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub name: String,
    pub arguments: Value,
    pub timestamp: DateTime<Utc>,
}

impl McpToolCall {
    pub fn new(name: &str, arguments: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            name: name.to_string(),
            arguments,
            timestamp: Utc::now(),
        }
    }
}

/// Représente le résultat de l'exécution d'un outil.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpToolResult {
    #[serde(rename = "_id")]
    pub id: Uuid,
    pub call_id: Uuid,
    pub content: Value,
    pub is_error: bool,
    pub timestamp: DateTime<Utc>,
}

impl McpToolResult {
    pub fn success(call_id: Uuid, content: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            call_id,
            content,
            is_error: false,
            timestamp: Utc::now(),
        }
    }

    pub fn error(call_id: Uuid, message: &str) -> Self {
        Self {
            id: Uuid::new_v4(),
            call_id,
            content: serde_json::json!({ "error": message }),
            is_error: true,
            timestamp: Utc::now(),
        }
    }
}

// =========================================================================
// 2. LOGIQUE D'EXÉCUTION (Registry & Traits) - Ajout pour rendre le MCP actif
// =========================================================================

/// Définition d'un outil exposée au LLM (Schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: Value, // JSON Schema des arguments
}

/// Trait que chaque outil concret (FileSystem, Search, etc.) devra implémenter.
#[async_trait]
pub trait McpTool: Send + Sync {
    /// Retourne la définition pour le Prompt Système
    fn definition(&self) -> ToolDefinition;

    /// Exécute l'outil en prenant un Call et en retournant un Result
    async fn execute(&self, call: McpToolCall) -> McpToolResult;
}

/// Catalogue d'outils disponibles pour un Agent.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn McpTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Enregistre un nouvel outil dans le registre.
    pub fn register<T: McpTool + 'static>(&mut self, tool: T) {
        let def = tool.definition();
        self.tools.insert(def.name, Arc::new(tool));
    }

    /// Récupère un outil par son nom.
    pub fn get(&self, name: &str) -> Option<Arc<dyn McpTool>> {
        self.tools.get(name).cloned()
    }

    /// Liste toutes les définitions (pour l'injection dans le prompt).
    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        self.tools.values().map(|t| t.definition()).collect()
    }

    /// Génère le texte à injecter dans le prompt système de l'agent.
    pub fn to_system_prompt(&self) -> String {
        if self.tools.is_empty() {
            return String::new();
        }

        let mut prompt = String::from("\n## OUTILS DISPONIBLES (Protocol MCP)\n");
        prompt.push_str("Tu as accès à des outils externes. Pour en utiliser un, réponds UNIQUEMENT avec ce format JSON :\n");
        prompt.push_str("```json\n{ \"mcp_tool_call\": { \"name\": \"tool_name\", \"arguments\": { ... } } }\n```\n\n");
        prompt.push_str("Liste des outils :\n");

        for tool in self.tools.values() {
            let def = tool.definition();
            prompt.push_str(&format!("- **{}**: {}\n", def.name, def.description));
            // On pourrait ajouter le schema complet ici si besoin, mais la description suffit souvent pour les modèles puissants.
        }
        prompt
    }
}

// =========================================================================
// 3. TESTS
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // --- Tests des Structures (Vos tests existants) ---
    #[test]
    fn test_mcp_tool_call_creation() {
        let args = json!({
            "path": "/tmp/test.txt",
            "content": "Hello World"
        });
        let call = McpToolCall::new("fs_write", args.clone());
        assert_eq!(call.name, "fs_write");
        assert_eq!(call.arguments["path"], "/tmp/test.txt");
        assert!(!call.id.is_nil());
    }

    #[test]
    fn test_mcp_result_serialization() {
        let call_id = Uuid::new_v4();
        let result = McpToolResult::success(call_id, json!("Operation successful"));
        let json_str = serde_json::to_string(&result).expect("Serialization failed");
        assert!(json_str.contains("_id"));
        assert!(json_str.contains("callId"));
        assert!(json_str.contains("isError"));
    }

    #[test]
    fn test_mcp_error_handling() {
        let call_id = Uuid::new_v4();
        let result = McpToolResult::error(call_id, "Access Denied");
        assert!(result.is_error);
        assert_eq!(result.content["error"], "Access Denied");
    }

    // --- Tests du Registre (Nouveau) ---

    // Outil Mock pour les tests
    struct EchoTool;
    #[async_trait]
    impl McpTool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                name: "echo".to_string(),
                description: "Renvoie l'argument".to_string(),
                input_schema: json!({ "type": "object", "properties": { "msg": { "type": "string" } } }),
            }
        }
        async fn execute(&self, call: McpToolCall) -> McpToolResult {
            McpToolResult::success(call.id, call.arguments)
        }
    }

    #[tokio::test]
    async fn test_registry_execution() {
        let mut registry = ToolRegistry::new();
        registry.register(EchoTool);

        let tool = registry.get("echo").expect("Tool not found");
        let call = McpToolCall::new("echo", json!({ "msg": "Hello MCP" }));

        let result = tool.execute(call).await;
        assert!(!result.is_error);
        assert_eq!(result.content["msg"], "Hello MCP");
    }
}
