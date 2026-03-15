// FICHIER : src-tauri/src/ai/tools/query_db.rs

use crate::ai::protocols::mcp::{McpTool, McpToolCall, McpToolResult, ToolDefinition};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::processor::JsonLdProcessor;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

/// Outil permettant à l'IA d'interroger le Graphe de Connaissances RAISE.
pub struct QueryDbTool {
    // On stocke le StorageEngine (via SharedRef) pour éviter les problèmes de lifetimes ('a)
    storage: SharedRef<StorageEngine>,
    space: String,
    db: String,
}

impl QueryDbTool {
    pub fn new(storage: SharedRef<StorageEngine>, space: String, db: String) -> Self {
        Self { storage, space, db }
    }
}

#[async_interface]
impl McpTool for QueryDbTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "query_db".to_string(),
            description: "Interroge le Graphe de Connaissances RAISE (JSON-LD). Permet de lire le contenu d'un noeud (Agent, Service, Prompt, etc.) à partir de son UUID ou de sa référence 'ref:...'.".to_string(),
            input_schema: json_value!({
                "type": "object",
                "required": ["reference"],
                "properties": {
                    "reference": {
                        "type": "string",
                        "description": "L'identifiant UUID ou la référence URN complète (ex: 'ref:agents:handle:agent_rust_dev')."
                    },
                    "collection": {
                        "type": "string",
                        "description": "Le nom de la collection. Optionnel si 'reference' est une URN 'ref:' complète."
                    },
                    "as_rdf": {
                        "type": "boolean",
                        "description": "Si true, retourne le graphe en format N-Triples. Utile pour l'inférence logique pure."
                    }
                }
            }),
        }
    }

    async fn execute(&self, call: McpToolCall) -> McpToolResult {
        // 1. Extraction sécurisée des arguments (Façade JSON)
        let reference = match call.arguments.get("reference").and_then(|v| v.as_str()) {
            Some(r) => r,
            None => {
                return McpToolResult::error(call.id, "Argument 'reference' manquant ou invalide")
            }
        };

        let collection_arg = call.arguments.get("collection").and_then(|v| v.as_str());
        let as_rdf = call
            .arguments
            .get("as_rdf")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 2. Instanciation éphémère du Manager
        // Le Manager est recréé "à la volée" lors de la requête pour respecter le lifetime &'a
        let manager = CollectionsManager::new(&self.storage, &self.space, &self.db);

        // 3. Parsing du Smart Link vs UUID
        let (target_col, field, val) = if reference.starts_with("ref:") {
            // Extrait col, field, et val depuis 'ref:collection:champ:valeur'
            let parts: Vec<&str> = reference.splitn(4, ':').collect();
            if parts.len() == 4 {
                (
                    parts[1].to_string(),
                    parts[2].to_string(),
                    parts[3].to_string(),
                )
            } else {
                return McpToolResult::error(
                    call.id,
                    "Format URN 'ref:' invalide. Attendu: ref:collection:champ:valeur",
                );
            }
        } else {
            let Some(col) = collection_arg else {
                return McpToolResult::error(
                    call.id,
                    "L'argument 'collection' est requis si 'reference' n'est pas une URN 'ref:'.",
                );
            };
            (col.to_string(), "_id".to_string(), reference.to_string())
        };

        // 4. Résolution et Requête vers JSON-DB
        let doc = if field == "_id" {
            // Lecture directe
            match manager.get_document(&target_col, &val).await {
                Ok(Some(d)) => d,
                Ok(None) => {
                    return McpToolResult::error(
                        call.id,
                        &format!("Entité UUID introuvable dans '{}'", target_col),
                    )
                }
                Err(e) => {
                    return McpToolResult::error(call.id, &format!("Erreur de lecture DB: {}", e))
                }
            }
        } else {
            // Requête dynamique (ex: par 'handle')
            let mut query = Query::new(&target_col);
            query.filter = Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq(&field, json_value!(val))],
            });
            query.limit = Some(1);

            let engine = QueryEngine::new(&manager);
            match engine.execute_query(query).await {
                Ok(res) => {
                    if let Some(d) = res.documents.first() {
                        d.clone()
                    } else {
                        return McpToolResult::error(
                            call.id,
                            &format!("Entité introuvable pour {}:{}", field, val),
                        );
                    }
                }
                Err(e) => {
                    return McpToolResult::error(call.id, &format!("Erreur de requête DB: {}", e))
                }
            }
        };

        // 5. Injection de la couche Sémantique (Ontologie)
        let processor = JsonLdProcessor::new();

        if as_rdf {
            match processor.to_ntriples(&doc) {
                Ok(rdf_triples) => McpToolResult::success(
                    call.id,
                    json_value!({ "format": "n-triples", "data": rdf_triples }),
                ),
                Err(e) => {
                    McpToolResult::error(call.id, &format!("Erreur de conversion RDF: {}", e))
                }
            }
        } else {
            let compacted_doc = processor.compact(&doc);
            McpToolResult::success(
                call.id,
                json_value!({ "format": "json-ld", "data": compacted_doc }),
            )
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox; // Import exact de la Sandbox

    #[async_test]
    async fn test_query_db_missing_args() {
        let sandbox = AgentDbSandbox::new().await;

        let tool = QueryDbTool::new(
            sandbox.db.clone(),
            sandbox.config.system_domain.clone(),
            sandbox.config.system_db.clone(),
        );

        let call = McpToolCall::new("query_db", json_value!({ "collection": "agents" }));
        let result = tool.execute(call).await;

        // On vérifie que le résultat est bien une erreur
        assert!(result.is_error);
    }
}
