// FICHIER : crates/raise-core/src/ai/tools/git_tool.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

#[derive(Debug, Serializable, Deserializable, Clone)]
pub struct TrafficStats {
    pub views: u64,
    pub unique_visitors: u64,
    pub timestamp: String,
}

pub struct GitTool {
    workspace_dir: PathBuf,
    tool_def: ToolDefinition,
}

impl GitTool {
    pub async fn new(
        workspace_dir: PathBuf,
        db: SharedRef<StorageEngine>,
        space: &str,
        db_name: &str,
    ) -> RaiseResult<Self> {
        let manager = CollectionsManager::new(&db, space, db_name);

        // 1. Lecture stricte du serveur MCP
        let mut query = Query::new("mcp_servers");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!("mcp_server_toolkit"))],
        });

        let mcp_config = match QueryEngine::new(&manager).execute_query(query).await {
            Ok(res) if !res.documents.is_empty() => res.documents[0].clone(),
            _ => raise_error!(
                "ERR_GIT_SERVER_MISSING",
                error = "Serveur MCP 'mcp_server_toolkit' introuvable."
            ),
        };

        // 2. Extraction stricte
        let tools = match mcp_config.get("tools").and_then(|t| t.as_array()) {
            Some(t) => t,
            None => raise_error!(
                "ERR_GIT_TOOLS_ARRAY_MISSING",
                error = "Tableau 'tools' absent."
            ),
        };

        let gt_tool = match tools
            .iter()
            .find(|t| t.get("tool_id").and_then(|v| v.as_str()) == Some("git_tool"))
        {
            Some(t) => t,
            None => raise_error!(
                "ERR_GIT_TOOL_NOT_FOUND",
                error = "L'outil 'git_tool' n'est pas déclaré."
            ),
        };

        let tool_name = match gt_tool.get("tool_id").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => raise_error!(
                "ERR_GIT_TOOL_ID_MISSING",
                error = "Propriété 'tool_id' manquante."
            ),
        };

        let tool_desc = match gt_tool.get("description").and_then(|v| v.as_str()) {
            Some(d) => d.to_string(),
            None => raise_error!(
                "ERR_GIT_TOOL_DESC_MISSING",
                error = "Propriété 'description' manquante."
            ),
        };

        let schema_uri = match gt_tool.get("input_schema_uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => raise_error!("ERR_GIT_INPUT_SCHEMA_URI_MISSING", error = "URI manquante."),
        };

        let input_schema = match manager.get_schema_def(schema_uri).await {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_GIT_INPUT_SCHEMA_RESOLUTION",
                error = format!("Impossible de résoudre {} : {}", schema_uri, e)
            ),
        };

        let tool_def = ToolDefinition {
            name: tool_name,
            description: tool_desc,
            input_schema,
        };
        Ok(Self {
            workspace_dir,
            tool_def,
        })
    }

    async fn execute_git(args: &[&str], cwd: &Path) -> RaiseResult<String> {
        let command_res = AsyncCommand::new("git")
            .args(args)
            .current_dir(cwd)
            .output()
            .await;
        match command_res {
            Ok(output) => {
                if output.status.success() {
                    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                    raise_error!(
                        "ERR_GIT_COMMAND_FAILED",
                        error = stderr,
                        context = json_value!({ "args": args })
                    )
                }
            }
            Err(e) => raise_error!("ERR_GIT_PROCESS_SPAWN", error = e),
        }
    }

    pub async fn secure_publish(cwd: &Path, message: &str) -> RaiseResult<String> {
        user_info!(
            "INF_GIT_PUBLISH_START",
            json_value!({ "path": cwd.to_string_lossy() })
        );
        let _ = Self::execute_git(&["add", "."], cwd).await?;

        let xai_id = UniqueId::new_v4();
        let full_msg = format!("ai(core): {} [XAI-Ref: {}]", message, xai_id);

        let _ = Self::execute_git(&["commit", "-m", &full_msg], cwd).await?;
        match Self::execute_git(&["push"], cwd).await {
            Ok(stdout) => {
                user_success!(
                    "SUC_GIT_PUBLISH_COMPLETE",
                    json_value!({ "commit_id": xai_id.to_string() })
                );
                Ok(stdout)
            }
            Err(e) => Err(e),
        }
    }

    pub async fn fetch_traffic(owner: &str, repo: &str, token: &str) -> RaiseResult<TrafficStats> {
        user_info!("INF_GIT_FETCH_TRAFFIC", json_value!({ "repo": repo }));

        let client = get_client();
        let url = format!(
            "https://api.github.com/repos/{}/{}/traffic/views",
            owner, repo
        );

        let response = match client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Accept", "application/vnd.github+json")
            .send()
            .await
        {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_GIT_NETWORK_FAILURE",
                error = e,
                context = json_value!({ "url": url })
            ),
        };

        if !response.status().is_success() {
            let status = response.status();
            raise_error!(
                "ERR_GIT_API_RESPONSE",
                error = format!("Status: {}", status)
            )
        }

        let body: JsonValue = match response.json().await {
            Ok(json) => json,
            Err(e) => raise_error!("ERR_GIT_JSON_DECODING", error = e),
        };

        let views = body
            .get("count")
            .and_then(|v| v.as_u64())
            .unwrap_or_default();
        let unique_visitors = body
            .get("uniques")
            .and_then(|v| v.as_u64())
            .unwrap_or_default();

        Ok(TrafficStats {
            views,
            unique_visitors,
            timestamp: UtcClock::now().to_rfc3339(),
        })
    }
}

#[async_interface]
impl McpTool for GitTool {
    fn definition(&self) -> ToolDefinition {
        self.tool_def.clone()
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        let action = match call.arguments.get("action").and_then(|v| v.as_str()) {
            Some(a) => a,
            None => return McpToolResult::error(call.id, "Argument 'action' manquant."),
        };

        match action {
            "status" => match Self::execute_git(&["status", "-s"], &self.workspace_dir).await {
                Ok(stdout) => McpToolResult::success(
                    call.id,
                    json_value!({ "status": "success", "stdout": stdout }),
                ),
                Err(e) => McpToolResult::error(call.id, &format!("Erreur 'git status': {}", e)),
            },
            "diff" => match Self::execute_git(&["diff"], &self.workspace_dir).await {
                Ok(stdout) => McpToolResult::success(
                    call.id,
                    json_value!({ "status": "success", "stdout": stdout }),
                ),
                Err(e) => McpToolResult::error(call.id, &format!("Erreur 'git diff': {}", e)),
            },
            "add" => match Self::execute_git(&["add", "."], &self.workspace_dir).await {
                Ok(stdout) => McpToolResult::success(
                    call.id,
                    json_value!({ "status": "success", "stdout": stdout }),
                ),
                Err(e) => McpToolResult::error(call.id, &format!("Erreur 'git add': {}", e)),
            },
            "commit" => {
                let msg = match call
                    .arguments
                    .get("commit_message")
                    .and_then(|v| v.as_str())
                {
                    Some(m) => m,
                    None => {
                        return McpToolResult::error(call.id, "Argument 'commit_message' requis.")
                    }
                };
                match Self::secure_publish(&self.workspace_dir, msg).await {
                    Ok(stdout) => McpToolResult::success(
                        call.id,
                        json_value!({ "status": "success", "stdout": stdout }),
                    ),
                    Err(e) => McpToolResult::error(call.id, &format!("Erreur 'git commit': {}", e)),
                }
            }
            _ => McpToolResult::error(call.id, "Action Git non supportée par le contrat."),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::config::AppConfig;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    async fn inject_mock_git_config(
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

        let input_uri = "v2/agents/tools/inputs/git_tool_input.schema.json";

        if with_schema {
            manager.create_schema_def(input_uri, json_value!({
                "type": "object",
                "properties": { "action": { "type": "string" }, "commit_message": { "type": "string" } }
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
                        "tool_id": "git_tool",
                        "description": "Git MCP Tool",
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
    async fn test_git_init_fails_if_schema_missing() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_git_config(&manager, false).await?;

        let result = GitTool::new(
            sandbox.domain_root.clone(),
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        )
        .await;

        match result {
            Err(e) if e.to_string().contains("ERR_GIT_INPUT_SCHEMA_RESOLUTION") => Ok(()),
            _ => panic!(
                "L'initialisation aurait dû échouer de manière stricte par manque de schéma."
            ),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_git_mcp_missing_arguments() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_git_config(&manager, true).await?;

        let tool = GitTool::new(
            sandbox.domain_root.clone(),
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        )
        .await?;

        let call_empty = McpToolCall::new("git_tool", json_value!({}));
        let res_empty = tool.execute(call_empty).await;
        assert!(res_empty.is_error);

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_git_publish_error_on_invalid_repo() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let invalid_path = sandbox.domain_root.join("not_a_repo");
        fs::ensure_dir_async(&invalid_path).await.unwrap();

        match GitTool::secure_publish(&invalid_path, "Test Fail").await {
            Ok(_) => panic!("Le test aurait dû échouer car le dossier n'est pas un dépôt Git"),
            Err(e) => assert!(format!("{:?}", e).contains("ERR_GIT_COMMAND_FAILED")),
        }
        Ok(())
    }
}
