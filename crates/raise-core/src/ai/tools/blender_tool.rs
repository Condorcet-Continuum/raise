// FICHIER : crates/raise-core/src/ai/tools/blender_tool.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;
use crate::workflow_engine::handlers::HandlerContext;
use crate::workflow_engine::tools::AgentTool;

/// Outil Agent "Data-Driven" permettant de piloter Blender.
#[derive(Debug)]
pub struct BlenderTool {
    dataset_dir: PathBuf,
    cached_name: String,
    cached_description: String,
    cached_schema: JsonValue,
    cached_output_schema: Option<JsonValue>,
    cmd: String,
    args: Vec<String>,
    template: String,
}

impl BlenderTool {
    /// Initialisation asynchrone ultra-stricte depuis le serveur MCP.
    pub async fn init(
        dataset_dir: PathBuf,
        db: &SharedRef<StorageEngine>,
        space: &str,
        db_name: &str,
        server_handle: &str, // 🎯 NOUVEAU : ID du serveur MCP parent (ex: "mcp_server_blender")
        tool_id: &str,
    ) -> RaiseResult<Self> {
        let manager = CollectionsManager::new(db, space, db_name);

        // 1. Lecture stricte
        let mut query = Query::new("mcp_servers");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!(server_handle))],
        });

        let config_doc = match QueryEngine::new(&manager).execute_query(query).await {
            Ok(res) if !res.documents.is_empty() => res.documents[0].clone(),
            _ => raise_error!(
                "ERR_BLENDER_SERVER_MISSING",
                error = format!("Serveur MCP '{}' introuvable.", server_handle)
            ),
        };

        // 2. Extraction stricte du tableau tools
        let tools = match config_doc.get("tools").and_then(|t| t.as_array()) {
            Some(t) => t,
            None => raise_error!(
                "ERR_BLENDER_TOOLS_MISSING",
                error = "Tableau 'tools' absent."
            ),
        };

        // 3. Identification stricte
        let tool_config = match tools
            .iter()
            .find(|t| t.get("tool_id").and_then(|v| v.as_str()) == Some(tool_id))
        {
            Some(t) => t,
            None => raise_error!(
                "ERR_BLENDER_TOOL_NOT_FOUND",
                error = format!("Outil '{}' introuvable.", tool_id)
            ),
        };

        let cached_name = match tool_config.get("tool_id").and_then(|id| id.as_str()) {
            Some(n) => n.to_string(),
            None => raise_error!("ERR_BLENDER_ID_MISSING", error = "tool_id manquant."),
        };

        let cached_description = match tool_config.get("description").and_then(|d| d.as_str()) {
            Some(d) => d.to_string(),
            None => raise_error!("ERR_BLENDER_DESC_MISSING", error = "description manquante."),
        };

        // 4. Résolution stricte des schémas
        let input_uri = match tool_config.get("input_schema_uri").and_then(|u| u.as_str()) {
            Some(u) => u,
            None => raise_error!(
                "ERR_BLENDER_INPUT_URI_MISSING",
                error = "input_schema_uri manquante."
            ),
        };

        let cached_schema = match manager.get_schema_def(input_uri).await {
            Ok(schema) => schema,
            Err(e) => raise_error!("ERR_BLENDER_INPUT_SCHEMA_FAIL", error = e),
        };

        let output_uri = match tool_config
            .get("output_schema_uri")
            .and_then(|u| u.as_str())
        {
            Some(u) => u,
            None => raise_error!(
                "ERR_BLENDER_OUTPUT_URI_MISSING",
                error = "output_schema_uri manquante."
            ),
        };

        let cached_output_schema = match manager.get_schema_def(output_uri).await {
            Ok(schema) => Some(schema),
            Err(e) => raise_error!("ERR_BLENDER_OUTPUT_SCHEMA_FAIL", error = e),
        };

        // 5. Caching strict de l'exécution OS
        let stdio = match config_doc.get("stdio") {
            Some(s) => s,
            None => raise_error!(
                "ERR_BLENDER_STDIO_MISSING",
                error = "Paramètres 'stdio' manquants."
            ),
        };

        let cmd = match stdio.get("command").and_then(|c| c.as_str()) {
            Some(c) => c.to_string(),
            None => raise_error!("ERR_BLENDER_CMD_MISSING", error = "Commande OS manquante."),
        };

        let args = match stdio.get("args").and_then(|a| a.as_array()) {
            Some(arr) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            None => raise_error!(
                "ERR_BLENDER_ARGS_MISSING",
                error = "Arguments OS manquants."
            ),
        };

        let template = match config_doc
            .get("prompts")
            .and_then(|p| p.as_array())
            .and_then(|a| a.first())
            .and_then(|p| p.get("content"))
            .and_then(|c| c.as_str())
        {
            Some(t) => t.to_string(),
            None => raise_error!(
                "ERR_BLENDER_TEMPLATE_MISSING",
                error = "Template de prompt manquant."
            ),
        };

        Ok(Self {
            dataset_dir,
            cached_name,
            cached_description,
            cached_schema,
            cached_output_schema,
            cmd,
            args,
            template,
        })
    }
}

#[async_interface]
impl AgentTool for BlenderTool {
    fn name(&self) -> &str {
        &self.cached_name
    }
    fn description(&self) -> &str {
        &self.cached_description
    }
    fn parameters_schema(&self) -> JsonValue {
        self.cached_schema.clone()
    }
    fn output_schema(&self) -> Option<JsonValue> {
        self.cached_output_schema.clone()
    }

    async fn execute(
        &self,
        params: &JsonValue,
        _context: &HandlerContext<'_>,
    ) -> RaiseResult<JsonValue> {
        let filename = match params.get("output_filename").and_then(|v| v.as_str()) {
            Some(f) => f,
            None => raise_error!(
                "ERR_BLENDER_ARG_MISSING",
                error = "Argument 'output_filename' requis."
            ),
        };
        let defect = match params.get("defect_type").and_then(|v| v.as_str()) {
            Some(d) => d,
            None => raise_error!(
                "ERR_BLENDER_ARG_MISSING",
                error = "Argument 'defect_type' requis."
            ),
        };
        let lighting = match params.get("lighting_intensity") {
            Some(l) => l.to_string(),
            None => raise_error!(
                "ERR_BLENDER_ARG_MISSING",
                error = "Argument 'lighting_intensity' requis."
            ),
        };

        if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
            raise_error!(
                "ERR_FS_SECURITY_VIOLATION",
                error = "Path traversal détecté."
            );
        }

        fs::ensure_dir_async(&self.dataset_dir).await?;
        let full_path = self.dataset_dir.join(filename);
        let path_str = full_path.to_string_lossy().to_string();

        let mut final_args = self.args.clone();
        let python_expr = self
            .template
            .replace("{{defect_type}}", defect)
            .replace("{{lighting_intensity}}", &lighting)
            .replace("{{output_path}}", &path_str);

        final_args.push(python_expr);

        let args_refs: Vec<&str> = final_args.iter().map(|s| s.as_str()).collect();
        match os::exec_command_async(&self.cmd, &args_refs, Some(&self.dataset_dir)).await {
            Ok(stdout) => Ok(
                json_value!({ "path": path_str, "status": "success", "blender_output": stdout.lines().last().unwrap_or("OK") }),
            ),
            Err(e) => Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    async fn seed_strict_blender_tool(
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

        let input_id = "v2/agents/tools/blender_input.schema.json";
        let output_id = "v2/agents/tools/blender_output.schema.json";

        if with_schema {
            manager
                .create_schema_def(input_id, json_value!({"type": "object"}))
                .await?;
            manager
                .create_schema_def(output_id, json_value!({"type": "object"}))
                .await?;
        }

        let input_uri = manager.build_schema_uri(input_id).await;
        let output_uri = manager.build_schema_uri(output_id).await;

        manager
            .upsert_document(
                "mcp_servers",
                json_value!({
                    "handle": "mcp_server_blender",
                    "@type": ["raise:McpTool", "pa:PhysicalFunction", "blender:SyntheticGenerator"],
                    "stdio": { "command": "blender", "args": ["-b"] },
                    "tools": [{
                        "tool_id": "gen_blender_data",
                        "description": "Blender API",
                        "input_schema_uri": input_uri,
                        "output_schema_uri": output_uri
                    }],
                    "prompts": [{ "content": "script={{defect_type}}" }]
                }),
            )
            .await?;

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_blender_init_success() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        seed_strict_blender_tool(&manager, true).await?;

        let dataset_dir = sandbox.domain_root.join("dataset");
        let tool = BlenderTool::init(
            dataset_dir,
            &sandbox.db,
            &manager.space,
            &manager.db,
            "mcp_server_blender",
            "gen_blender_data",
        )
        .await?;

        assert_eq!(tool.name(), "gen_blender_data");
        Ok(())
    }
}
