// FICHIER : src-tauri/src/ai/tools/file_system.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::utils::{async_trait, io::PathBuf, prelude::*};
use std::fs;

/// Outil permettant à l'IA d'écrire un fichier sur le disque.
pub struct FileWriteTool {
    root_dir: PathBuf, // Sécurité : on restreint l'écriture à un dossier racine
}

impl FileWriteTool {
    pub fn new(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }
}

#[async_trait]
impl McpTool for FileWriteTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "fs_write".to_string(),
            description:
                "Écrit du contenu texte dans un fichier. Crée les dossiers parents si nécessaire."
                    .to_string(),
            input_schema: json!({
                "type": "object",
                "required": ["path", "content"],
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Chemin relatif du fichier (ex: src/main.rs)"
                    },
                    "content": {
                        "type": "string",
                        "description": "Contenu complet du fichier"
                    }
                }
            }),
        }
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        // 1. Extraction des arguments
        let relative_path = match call.arguments["path"].as_str() {
            Some(p) => p,
            None => return McpToolResult::error(call.id, "Argument 'path' manquant ou invalide"),
        };

        let content = match call.arguments["content"].as_str() {
            Some(c) => c,
            None => {
                return McpToolResult::error(call.id, "Argument 'content' manquant ou invalide")
            }
        };

        // 2. Sécurisation du chemin
        if relative_path.contains("..")
            || relative_path.starts_with("/")
            || relative_path.starts_with("\\")
        {
            return McpToolResult::error(
                call.id,
                "Chemin invalide : Doit être relatif et sans '..'",
            );
        }

        let full_path = self.root_dir.join(relative_path);

        // 3. Création des dossiers parents
        if let Some(parent) = full_path.parent() {
            if let Err(e) = fs::create_dir_all(parent) {
                return McpToolResult::error(
                    call.id,
                    &format!("Impossible de créer le dossier parent: {}", e),
                );
            }
        }

        // 4. Écriture du fichier
        match fs::write(&full_path, content) {
            Ok(_) => {
                McpToolResult::success(call.id, json!({ "status": "success", "path": full_path }))
            }
            Err(e) => McpToolResult::error(call.id, &format!("Erreur d'écriture: {}", e)),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::tempdir;

    fn make_call(path: &str, content: &str) -> McpToolCall {
        McpToolCall::new(
            "fs_write",
            json!({
                "path": path,
                "content": content
            }),
        )
    }

    #[tokio::test]
    async fn test_write_file_simple() {
        let dir = tempdir().unwrap();
        let tool = FileWriteTool::new(dir.path().to_path_buf());
        let call = make_call("hello.txt", "Hello World");
        let result = tool.execute(call).await;
        assert!(!result.is_error);
        let file_path = dir.path().join("hello.txt");
        assert!(file_path.exists());
        let saved_content = fs::read_to_string(file_path).unwrap();
        assert_eq!(saved_content, "Hello World");
    }

    #[tokio::test]
    async fn test_write_nested_directories() {
        let dir = tempdir().unwrap();
        let tool = FileWriteTool::new(dir.path().to_path_buf());
        let call = make_call("src/components/button.rs", "struct Button;");
        let result = tool.execute(call).await;
        assert!(!result.is_error);
        let file_path = dir.path().join("src/components/button.rs");
        assert!(file_path.exists());
    }

    #[tokio::test]
    async fn test_security_prevent_path_traversal() {
        let dir = tempdir().unwrap();
        let tool = FileWriteTool::new(dir.path().to_path_buf());
        let call = make_call("../secret.txt", "hacked");
        let result = tool.execute(call).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_security_prevent_absolute_path() {
        let dir = tempdir().unwrap();
        let tool = FileWriteTool::new(dir.path().to_path_buf());
        let call = make_call("/etc/passwd", "hacked");
        let result = tool.execute(call).await;
        assert!(result.is_error);
    }

    #[tokio::test]
    async fn test_missing_arguments() {
        let dir = tempdir().unwrap();
        let tool = FileWriteTool::new(dir.path().to_path_buf());
        let call = McpToolCall::new("fs_write", json!({ "path": "test.txt" }));
        let result = tool.execute(call).await;
        assert!(result.is_error);
    }
}
