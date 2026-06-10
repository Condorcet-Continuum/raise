// FICHIER : crates/raise-core/src/ai/tools/file_system.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

/// Outil permettant à l'IA d'interagir avec le système de fichiers (Read/Write/List/Delete).
#[derive(Debug)]
pub struct FileSystemTool {
    root_dir: PathBuf,
    tool_def: ToolDefinition,
}

impl FileSystemTool {
    /// Initialise l'outil dynamiquement depuis la collection des serveurs MCP (Zéro Fallback).
    pub async fn new(
        root_dir: PathBuf,
        db: SharedRef<StorageEngine>,
        space: &str,
        db_name: &str,
    ) -> RaiseResult<Self> {
        let manager = CollectionsManager::new(&db, space, db_name);

        // 1. Lecture stricte depuis la collection autonome mcp_servers
        let mut query = Query::new("mcp_servers");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!("mcp_server_toolkit"))],
        });

        let mcp_config = match QueryEngine::new(&manager).execute_query(query).await {
            Ok(res) if !res.documents.is_empty() => res.documents[0].clone(),
            _ => raise_error!(
                "ERR_FS_SERVER_MISSING",
                error = "Serveur MCP 'mcp_server_toolkit' introuvable dans la base."
            ),
        };

        // 2. Extraction stricte du tableau d'outils
        let tools = match mcp_config.get("tools").and_then(|t| t.as_array()) {
            Some(t) => t,
            None => raise_error!(
                "ERR_FS_TOOLS_ARRAY_MISSING",
                error = "Le tableau 'tools' est absent de la configuration du serveur."
            ),
        };

        // 3. Recherche stricte de l'outil file_system
        let fs_tool = match tools
            .iter()
            .find(|t| t.get("tool_id").and_then(|v| v.as_str()) == Some("file_system"))
        {
            Some(t) => t,
            None => raise_error!(
                "ERR_FS_TOOL_NOT_FOUND",
                error = "L'outil 'file_system' n'est pas déclaré dans le tableau 'tools'."
            ),
        };

        // 4. Extraction stricte des métadonnées
        let tool_name = match fs_tool.get("tool_id").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => raise_error!(
                "ERR_FS_TOOL_ID_MISSING",
                error = "Propriété 'tool_id' manquante."
            ),
        };

        let tool_desc = match fs_tool.get("description").and_then(|v| v.as_str()) {
            Some(d) => d.to_string(),
            None => raise_error!(
                "ERR_FS_TOOL_DESC_MISSING",
                error = "Propriété 'description' manquante."
            ),
        };

        let schema_uri = match fs_tool.get("input_schema_uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => raise_error!(
                "ERR_FS_INPUT_SCHEMA_URI_MISSING",
                error = "Propriété 'input_schema_uri' manquante."
            ),
        };

        // 5. Résolution physique stricte du contrat JSON-Schema
        let input_schema = match manager.get_schema_def(schema_uri).await {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_FS_INPUT_SCHEMA_RESOLUTION",
                error = format!("Impossible de résoudre le schéma {} : {}", schema_uri, e)
            ),
        };

        let tool_def = ToolDefinition {
            name: tool_name,
            description: tool_desc,
            input_schema,
        };

        Ok(Self { root_dir, tool_def })
    }
}

#[async_interface]
impl McpTool for FileSystemTool {
    fn definition(&self) -> ToolDefinition {
        self.tool_def.clone()
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        let action = match call.arguments.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return McpToolResult::error(call.id, "Argument 'action' manquant."),
        };

        let relative_path = match call.arguments.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return McpToolResult::error(call.id, "Argument 'path' manquant."),
        };

        // SÉCURITÉ ABSOLUE
        if relative_path.contains("..")
            || relative_path.starts_with('/')
            || relative_path.starts_with('\\')
        {
            return McpToolResult::error(
                call.id,
                "Violation de sécurité : Chemin absolu ou '..' interdit.",
            );
        }

        let full_path = self.root_dir.join(relative_path);

        match action {
            "write" => {
                let content = match call.arguments.get("content").and_then(|v| v.as_str()) {
                    Some(c) => c,
                    None => {
                        return McpToolResult::error(
                            call.id,
                            "Argument 'content' manquant pour l'action write.",
                        )
                    }
                };

                if let Some(parent) = full_path.parent() {
                    let _ = fs::create_dir_all_async(parent).await;
                }
                match fs::write_async(&full_path, content).await {
                    Ok(_) => McpToolResult::success(
                        call.id,
                        json_value!({ "status": "success", "message": "Fichier écrit." }),
                    ),
                    Err(e) => McpToolResult::error(call.id, &format!("Erreur d'écriture: {}", e)),
                }
            }
            "read" => match fs::read_to_string_async(&full_path).await {
                Ok(content) => McpToolResult::success(
                    call.id,
                    json_value!({ "status": "success", "data": content }),
                ),
                Err(e) => McpToolResult::error(call.id, &format!("Erreur de lecture: {}", e)),
            },
            "list" => match fs::read_dir_async(&full_path).await {
                Ok(mut entries) => {
                    let mut files = vec![];
                    while let Ok(Some(entry)) = entries.next_entry().await {
                        if let Ok(name) = entry.file_name().into_string() {
                            files.push(name);
                        }
                    }
                    McpToolResult::success(
                        call.id,
                        json_value!({ "status": "success", "data": files }),
                    )
                }
                Err(e) => McpToolResult::error(call.id, &format!("Erreur de listage: {}", e)),
            },
            "delete" => {
                if full_path.is_file() {
                    let _ = fs::remove_file_async(&full_path).await;
                } else if full_path.is_dir() {
                    let _ = fs::remove_dir_all_async(&full_path).await;
                }
                McpToolResult::success(
                    call.id,
                    json_value!({ "status": "success", "message": "Élément supprimé." }),
                )
            }
            _ => McpToolResult::error(call.id, "Action non reconnue par le schéma."),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Robustes et Zéro Dette)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::config::AppConfig;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    async fn inject_mock_fs_config(
        manager: &CollectionsManager<'_>,
        with_schema: bool,
    ) -> RaiseResult<()> {
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            manager.space, manager.db
        );
        let _ = DbSandbox::mock_db(manager).await;

        manager
            .create_collection("mcp_servers", &generic_schema)
            .await?;
        manager
            .create_collection("schemas", &generic_schema)
            .await?;

        let input_uri = "v2/agents/tools/inputs/file_system_input.schema.json";

        if with_schema {
            manager.create_schema_def(input_uri, json_value!({
                "type": "object",
                "properties": { "action": { "type": "string" }, "path": { "type": "string" } }
            })).await?;
        }

        let full_uri = manager.build_schema_uri(input_uri).await;

        manager
            .upsert_document(
                "mcp_servers",
                json_value!({
                    "handle": "mcp_server_toolkit",
                    "@type": ["raise:McpTool", "pa:PhysicalFunction", "raise:AiToolkit"],
                    "transport": "stdio",
                    "tools": [{
                        "tool_id": "file_system",
                        "description": "FS Tool",
                        "input_schema_uri": full_uri
                    }]
                }),
            )
            .await?;

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_fs_init_fails_if_schema_missing() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_fs_config(&manager, false).await?;

        let result = FileSystemTool::new(
            sandbox.domain_root.clone(),
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        )
        .await;

        match result {
            Err(e) if e.to_string().contains("ERR_FS_INPUT_SCHEMA_RESOLUTION") => Ok(()),
            _ => panic!(
                "L'initialisation aurait dû échouer sans schéma : {:?}",
                result
            ),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_fs_execute_actions() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_fs_config(&manager, true).await?;

        let tool = FileSystemTool::new(
            sandbox.domain_root.clone(),
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        )
        .await?;

        let call_write = McpToolCall::new(
            "file_system",
            json_value!({ "action": "write", "path": "test.txt", "content": "Hello" }),
        );
        let res_write = tool.execute(call_write).await;
        assert!(!res_write.is_error);

        let call_read = McpToolCall::new(
            "file_system",
            json_value!({ "action": "read", "path": "test.txt" }),
        );
        let res_read = tool.execute(call_read).await;
        assert!(!res_read.is_error);
        assert_eq!(res_read.content["data"].as_str().unwrap(), "Hello");

        Ok(())
    }
}
