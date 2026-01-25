use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Représente une demande d'exécution d'outil (Function Call).
/// Cette structure est le "payload" technique qui sera transporté ou validé.
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct McpToolCall {
    /// Identifiant unique de l'appel.
    #[serde(rename = "_id")]
    pub id: Uuid,

    /// Le nom de l'outil ciblé (ex: "fs_write", "git_commit").
    pub name: String,

    /// Les arguments de la fonction sous format JSON.
    pub arguments: Value,

    /// Horodatage de la création de la demande.
    pub timestamp: DateTime<Utc>,
}

impl McpToolCall {
    /// Crée un nouvel appel d'outil.
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
    /// Identifiant unique du résultat.
    #[serde(rename = "_id")]
    pub id: Uuid,

    /// Référence à l'ID du `McpToolCall` qui a déclenché ce résultat.
    pub call_id: Uuid,

    /// Le contenu de la réponse (peut être une donnée, un texte, ou une erreur).
    pub content: Value,

    /// Flag indiquant si l'exécution a échoué (exit code != 0 ou exception).
    pub is_error: bool,

    /// Horodatage de la fin d'exécution.
    pub timestamp: DateTime<Utc>,
}

impl McpToolResult {
    /// Crée un résultat de succès.
    pub fn success(call_id: Uuid, content: Value) -> Self {
        Self {
            id: Uuid::new_v4(),
            call_id,
            content,
            is_error: false,
            timestamp: Utc::now(),
        }
    }

    /// Crée un résultat d'erreur.
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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

        // Vérification des conventions JSON
        assert!(json_str.contains("_id"));
        assert!(json_str.contains("callId")); // camelCase check
        assert!(json_str.contains("isError"));
        assert!(json_str.contains("false"));
    }

    #[test]
    fn test_mcp_error_handling() {
        let call_id = Uuid::new_v4();
        let result = McpToolResult::error(call_id, "Access Denied");

        assert!(result.is_error);
        assert_eq!(result.content["error"], "Access Denied");
    }
}
