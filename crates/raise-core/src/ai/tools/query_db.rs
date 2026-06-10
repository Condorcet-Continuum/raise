// FICHIER : crates/raise-core/src/ai/tools/query_db.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::processor::JsonLdProcessor;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

pub struct QueryDbTool {
    storage: SharedRef<StorageEngine>,
    space: String,
    db: String,
    tool_def: ToolDefinition,
}

impl QueryDbTool {
    pub async fn new(
        storage: SharedRef<StorageEngine>,
        space: &str,
        db_name: &str,
    ) -> RaiseResult<Self> {
        let manager = CollectionsManager::new(&storage, space, db_name);

        // 1. Lecture stricte du serveur MCP parent
        let mut query = Query::new("mcp_servers");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("handle", json_value!("mcp_server_toolkit"))],
        });

        let mcp_config = match QueryEngine::new(&manager).execute_query(query).await {
            Ok(res) if !res.documents.is_empty() => res.documents[0].clone(),
            _ => raise_error!(
                "ERR_QUERY_DB_SERVER_MISSING",
                error = "Serveur MCP 'mcp_server_toolkit' introuvable."
            ),
        };

        // 2. Extraction stricte
        let tools = match mcp_config.get("tools").and_then(|t| t.as_array()) {
            Some(t) => t,
            None => raise_error!(
                "ERR_QUERY_DB_TOOLS_MISSING",
                error = "Le tableau 'tools' est absent."
            ),
        };

        let q_tool = match tools
            .iter()
            .find(|t| t.get("tool_id").and_then(|v| v.as_str()) == Some("query_db"))
        {
            Some(t) => t,
            None => raise_error!(
                "ERR_QUERY_DB_TOOL_NOT_FOUND",
                error = "L'outil 'query_db' n'est pas déclaré."
            ),
        };

        let tool_name = match q_tool.get("tool_id").and_then(|v| v.as_str()) {
            Some(n) => n.to_string(),
            None => raise_error!(
                "ERR_QUERY_DB_ID_MISSING",
                error = "Propriété 'tool_id' manquante."
            ),
        };

        let tool_desc = match q_tool.get("description").and_then(|v| v.as_str()) {
            Some(d) => d.to_string(),
            None => raise_error!(
                "ERR_QUERY_DB_DESC_MISSING",
                error = "Propriété 'description' manquante."
            ),
        };

        let schema_uri = match q_tool.get("input_schema_uri").and_then(|v| v.as_str()) {
            Some(u) => u,
            None => raise_error!(
                "ERR_QUERY_DB_URI_MISSING",
                error = "Propriété 'input_schema_uri' manquante."
            ),
        };

        let input_schema = match manager.get_schema_def(schema_uri).await {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_QUERY_DB_SCHEMA_RESOLUTION",
                error = format!("Impossible de résoudre {} : {}", schema_uri, e)
            ),
        };

        let tool_def = ToolDefinition {
            name: tool_name,
            description: tool_desc,
            input_schema,
        };

        Ok(Self {
            storage,
            space: space.to_string(),
            db: db_name.to_string(),
            tool_def,
        })
    }
}

#[async_interface]
impl McpTool for QueryDbTool {
    fn definition(&self) -> ToolDefinition {
        self.tool_def.clone()
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        let reference = match call.arguments.get("reference").and_then(|v| v.as_str()) {
            Some(r) => r,
            None => return McpToolResult::error(call.id, "Argument 'reference' manquant."),
        };

        let collection_arg = call.arguments.get("collection").and_then(|v| v.as_str());
        let as_rdf = call
            .arguments
            .get("as_rdf")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let manager = CollectionsManager::new(&self.storage, &self.space, &self.db);

        let (target_col, field, val) = if reference.starts_with("ref:") {
            let parts: Vec<&str> = reference.splitn(4, ':').collect();
            match parts.len() {
                4 => (
                    parts[1].to_string(),
                    parts[2].to_string(),
                    parts[3].to_string(),
                ),
                _ => {
                    return McpToolResult::error(
                        call.id,
                        "Format URN invalide. Attendu: ref:col:champ:val",
                    )
                }
            }
        } else {
            match collection_arg {
                Some(col) => (col.to_string(), "_id".to_string(), reference.to_string()),
                None => {
                    return McpToolResult::error(
                        call.id,
                        "Argument 'collection' requis pour la recherche par UUID brut.",
                    )
                }
            }
        };

        let doc_res = if field == "_id" {
            match manager.get_document(&target_col, &val).await {
                Ok(doc) => Ok(doc),
                Err(e) => Err(build_error!("ERR_DB_READ", error = e.to_string())),
            }
        } else {
            let mut query = Query::new(&target_col);
            query.filter = Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq(&field, json_value!(val))],
            });
            query.limit = Some(1);

            let engine = QueryEngine::new(&manager);
            match engine.execute_query(query).await {
                Ok(res) => Ok(res.documents.first().cloned()),
                Err(e) => Err(build_error!("ERR_DB_QUERY", error = e.to_string())),
            }
        };

        match doc_res {
            Ok(Some(mut doc)) => {
                let processor = match JsonLdProcessor::new() {
                    Ok(p) => p,
                    Err(e) => {
                        return McpToolResult::error(
                            call.id,
                            &format!("Erreur d'initialisation du processeur sémantique: {}", e),
                        )
                    }
                };
                if as_rdf {
                    match processor.to_ntriples(&mut doc) {
                        Ok(triples) => McpToolResult::success(
                            call.id,
                            json_value!({"format": "n-triples", "data": triples}),
                        ),
                        Err(e) => {
                            McpToolResult::error(call.id, &format!("Erreur conversion RDF: {}", e))
                        }
                    }
                } else {
                    processor.compact_in_place(&mut doc);
                    McpToolResult::success(call.id, json_value!({"format": "json-ld", "data": doc}))
                }
            }
            Ok(None) => McpToolResult::error(
                call.id,
                "Entité introuvable dans le Graphe de Connaissances.",
            ),
            Err(e) => McpToolResult::error(call.id, &e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::data::config::AppConfig;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    async fn inject_mock_query_config(
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

        let input_uri = "v2/agents/tools/inputs/query_db_input.schema.json";

        if with_schema {
            manager
                .create_schema_def(
                    input_uri,
                    json_value!({
                        "type": "object",
                        "properties": { "reference": { "type": "string" } },
                        "required": ["reference"]
                    }),
                )
                .await?;
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
                        "tool_id": "query_db",
                        "description": "Mocked Descriptor",
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
    async fn test_query_db_init_fails_if_schema_missing() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        inject_mock_query_config(&manager, false).await?;

        let result = QueryDbTool::new(
            sandbox.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        )
        .await;

        match result {
            Err(e) if e.to_string().contains("ERR_QUERY_DB_SCHEMA_RESOLUTION") => Ok(()),
            _ => panic!("L'initialisation aurait dû échouer par manque de schéma."),
        }
    }
}
