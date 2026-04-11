// FICHIER : src-tauri/src/ai/tools/query_db.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::processor::JsonLdProcessor;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

/// Outil permettant à l'IA d'interroger le Graphe de Connaissances RAISE.
/// Gère la résolution des URN (ref:...) et la conversion RDF.
pub struct QueryDbTool {
    storage: SharedRef<StorageEngine>,
    space: String,
    db: String,
}

impl QueryDbTool {
    /// Initialise l'outil avec les coordonnées de la base cible.
    pub fn new(storage: SharedRef<StorageEngine>, space: String, db: String) -> Self {
        Self { storage, space, db }
    }
}

#[async_interface]
impl McpTool for QueryDbTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "query_db".to_string(),
            description: "Interroge le Graphe de Connaissances RAISE. Résout les UUID ou les URN 'ref:collection:champ:valeur'.".to_string(),
            input_schema: json_value!({
                "type": "object",
                "required": ["reference"],
                "properties": {
                    "reference": {
                        "type": "string",
                        "description": "UUID ou URN complète (ex: 'ref:agents:handle:dev_bot')."
                    },
                    "collection": {
                        "type": "string",
                        "description": "Nom de la collection (requis si reference est un UUID)."
                    },
                    "as_rdf": {
                        "type": "boolean",
                        "description": "Retourne le format N-Triples pour l'inférence logique."
                    }
                }
            }),
        }
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        // 1. Extraction sécurisée via Match
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

        // 2. Résolution du point de montage (Mount Point Resilience)
        let manager = CollectionsManager::new(&self.storage, &self.space, &self.db);

        // 3. Parsing sémantique de la référence (Match strict)
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
                        "Argument 'collection' requis pour recherche par UUID.",
                    )
                }
            }
        };

        // 4. Exécution de la requête avec Match...raise_error
        let doc_res = if field == "_id" {
            match manager.get_document(&target_col, &val).await {
                Ok(doc) => Ok(doc),
                Err(e) => Err(build_error!(
                    "ERR_DB_READ",
                    error = e.to_string(),
                    context = json_value!({"col": target_col, "id": val})
                )),
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

        // 5. Traitement du résultat et conversion sémantique
        match doc_res {
            Ok(Some(doc)) => {
                let processor = JsonLdProcessor::new();
                if as_rdf {
                    match processor.to_ntriples(&doc) {
                        Ok(triples) => McpToolResult::success(
                            call.id,
                            json_value!({"format": "n-triples", "data": triples}),
                        ),
                        Err(e) => {
                            McpToolResult::error(call.id, &format!("Erreur conversion RDF: {}", e))
                        }
                    }
                } else {
                    McpToolResult::success(
                        call.id,
                        json_value!({"format": "json-ld", "data": processor.compact(&doc)}),
                    )
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

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience des Mount Points)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Erreur si arguments manquants
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_query_db_missing_args() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let tool = QueryDbTool::new(
            sandbox.db.clone(),
            config.mount_points.system.domain.clone(),
            config.mount_points.system.db.clone(),
        );

        let call = McpToolCall::new("query_db", json_value!({ "collection": "agents" }));
        let result = tool.execute(call).await;

        assert!(
            result.is_error,
            "L'outil devrait échouer sans l'argument 'reference'"
        );
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience URN mal formée
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_query_db_resilience_bad_urn() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let tool = QueryDbTool::new(
            sandbox.db.clone(),
            config.mount_points.system.domain.clone(),
            config.mount_points.system.db.clone(),
        );

        let call = McpToolCall::new("query_db", json_value!({ "reference": "ref:too:short" }));
        let result = tool.execute(call).await;

        assert!(result.content["error"]
            .as_str()
            .unwrap()
            .contains("Format URN invalide"));
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Inférence Mount Point (System Domain)
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_query_db_mount_point_resolution() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // On vérifie que l'outil utilise bien les domaines configurés dynamiquement dans AppConfig
        let tool = QueryDbTool::new(
            sandbox.db.clone(),
            config.mount_points.system.domain.clone(),
            config.mount_points.system.db.clone(),
        );

        assert_eq!(tool.space, config.mount_points.system.domain);
        assert_eq!(tool.db, config.mount_points.system.db);
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à une collection inexistante
    #[async_test]
    async fn test_query_db_non_existent_collection() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let tool = QueryDbTool::new(
            sandbox.db.clone(),
            config.mount_points.system.domain.clone(),
            config.mount_points.system.db.clone(),
        );

        let call = McpToolCall::new("query_db", json_value!({ "reference": "ref:ghost:id:123" }));
        let result = tool.execute(call).await;

        // L'erreur doit être capturée par le match sur manager.get_document ou query_engine
        assert!(result.is_error);
        Ok(())
    }
}
