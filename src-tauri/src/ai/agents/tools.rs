// FICHIER : src-tauri/src/ai/agents/tools.rs

use super::{AgentContext, AgentSession, CreatedArtifact};
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*;

// Imports pour le protocole MCP et l'outil QueryDbTool
use crate::ai::protocols::mcp::{McpTool, McpToolCall};
use crate::ai::tools::QueryDbTool;

// Imports pour le moteur de base de données JSON-DB
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};

/// Extrait proprement du JSON depuis une réponse LLM, même s'il y a du markdown autour.
pub fn extract_json_from_llm(response: &str) -> String {
    let text = response.trim();
    let text = text
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let start = text.find('{').unwrap_or(0);
    let end = text.rfind('}').map(|i| i + 1).unwrap_or(text.len());

    if end > start {
        text[start..end].to_string()
    } else {
        text.to_string()
    }
}

/// Sauvegarde un artefact métier (composant LA, PA, etc.) dans le système de fichiers.
pub async fn save_artifact(
    ctx: &AgentContext,
    layer: &str,
    collection: &str,
    doc: &JsonValue,
) -> RaiseResult<CreatedArtifact> {
    let Some(doc_id_ref) = doc
        .get("_id")
        .or_else(|| doc.get("id"))
        .and_then(|v| v.as_str())
    else {
        raise_error!(
            "ERR_ARTIFACT_ID_INVALID",
            error = "L'artefact n'a pas d'ID valide",
            context = json_value!({ "doc_snapshot": doc })
        );
    };
    let doc_id = doc_id_ref.to_string();

    let name = doc["name"].as_str().unwrap_or("Unnamed").to_string();
    let element_type = doc
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("UnknownElement")
        .to_string();

    let relative_path = format!(
        "un2/{}/collections/{}/{}.json",
        layer.to_lowercase(),
        collection,
        doc_id
    );

    let full_path = ctx.paths.domain_root.join(&relative_path);

    if let Some(parent) = full_path.parent() {
        fs::create_dir_all_async(parent).await?;
    }

    let content = json::serialize_to_string_pretty(doc)?;
    fs::write_async(&full_path, content).await?;

    Ok(CreatedArtifact {
        id: doc_id,
        name,
        layer: layer.to_uppercase(),
        element_type,
        path: relative_path,
    })
}

/// Interroge le Graphe de Connaissances (JSON-LD) de manière centralisée.
pub async fn query_knowledge_graph(
    ctx: &AgentContext,
    reference: &str,
    as_rdf: bool,
) -> RaiseResult<JsonValue> {
    let config = AppConfig::get();
    let tool = QueryDbTool::new(
        ctx.db.clone(),
        config.system_domain.clone(),
        config.system_db.clone(),
    );

    let call = McpToolCall::new(
        "query_db",
        json_value!({
            "reference": reference,
            "as_rdf": as_rdf
        }),
    );

    let result = tool.execute(call).await;

    if result.is_error {
        let err_msg = result.content.as_str().unwrap_or("Erreur inconnue");
        raise_error!(
            "ERR_AGENT_QUERY_DB_FAIL",
            error = format!("Erreur lors de la lecture du graphe : {}", err_msg),
            context = json_value!({ "target_reference": reference })
        );
    }

    Ok(result.content["data"].clone())
}

/// Recherche un élément par son nom (Legacy Smart Linking).
pub async fn find_element_by_name(ctx: &AgentContext, name: &str) -> Option<JsonValue> {
    let config = AppConfig::get();
    let space = &config.system_domain;
    let db_name = &config.system_db;

    let manager = CollectionsManager::new(&ctx.db, space, db_name);
    let query_engine = QueryEngine::new(&manager);

    let collections = [
        "pa_components",
        "la_components",
        "sa_components",
        "functions",
        "actors",
        "capabilities",
    ];

    for col in collections {
        let mut query = Query::new(col);
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("name", name.into())],
        });
        query.limit = Some(1);

        if let Ok(result) = query_engine.execute_query(query).await {
            if let Some(doc) = result.documents.first() {
                return Some(doc.clone());
            }
        }
    }
    None
}

/// Charge la session de l'agent.
pub async fn load_session(ctx: &AgentContext) -> RaiseResult<AgentSession> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);

    let _ = manager
        .create_collection(
            "agent_sessions",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;

    match manager
        .get_document("agent_sessions", &ctx.session_id)
        .await
    {
        Ok(Some(doc_value)) => {
            let session: AgentSession = json::deserialize_from_value(doc_value)?;
            Ok(session)
        }
        _ => {
            let session = AgentSession::new(&ctx.session_id, &ctx.agent_id);
            save_session(ctx, &session).await?;
            Ok(session)
        }
    }
}

/// Sauvegarde la session de l'agent.
pub async fn save_session(ctx: &AgentContext, session: &AgentSession) -> RaiseResult<()> {
    let config = AppConfig::get();
    let manager = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);
    let json_doc = json::serialize_to_value(session)?;
    manager.upsert_document("agent_sessions", json_doc).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_clean() {
        let input = r#"{"key": "value"}"#;
        assert_eq!(extract_json_from_llm(input), input);
    }

    #[test]
    fn test_extract_json_markdown() {
        // Trick to avoid backtick issues in the generated file
        let bt = "```";
        let input = format!("{}json\n{{\"key\": \"value\"}}\n{}", bt, bt);
        assert_eq!(extract_json_from_llm(&input), "{\"key\": \"value\"}");
    }

    #[test]
    fn test_extract_json_noisy() {
        let bt = "```";
        let input = format!(
            "Texte avant\n{}json\n{{\"key\": \"value\"}}\n{}\nTexte après",
            bt, bt
        );
        assert_eq!(extract_json_from_llm(&input), "{\"key\": \"value\"}");
    }
}
