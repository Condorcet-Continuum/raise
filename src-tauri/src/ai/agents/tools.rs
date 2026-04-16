// FICHIER : src-tauri/src/ai/agents/tools.rs

use super::{AgentContext, AgentSession, CreatedArtifact};
use crate::ai::protocols::mcp::{McpTool, McpToolCall};
use crate::ai::tools::QueryDbTool;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*;

/// Extrait proprement un bloc JSON d'une réponse LLM (nettoyage Markdown)
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

/// 🎯 NOUVEAU : Sauvegarde en lot des artefacts via `insert_with_schema` pour garantir la validation
pub async fn save_artifacts_batch(
    ctx: &AgentContext,
    docs: Vec<JsonValue>,
) -> RaiseResult<Vec<CreatedArtifact>> {
    let mut artifacts = Vec::new();

    if docs.is_empty() {
        return Ok(artifacts);
    }

    // 1. Résolution du mapping via le Knowledge Graph
    let mapping_doc =
        match query_knowledge_graph(ctx, "ref:configs:handle:ontological_mapping", false).await {
            Ok(d) => d,
            Err(e) => raise_error!(
                "ERR_MISSING_ONTOLOGY_MAPPING",
                error = "Le document de configuration 'ontological_mapping' est introuvable.",
                context = json_value!({ "technical_error": e.to_string() })
            ),
        };

    let config = AppConfig::get();
    let sys_mgr = CollectionsManager::new(
        &ctx.db,
        &config.mount_points.system.domain,
        &config.mount_points.system.db,
    );

    let settings = match AppConfig::get_component_settings(&sys_mgr, "ai_agents").await {
        Ok(s) => s,
        Err(_) => json_value!({}),
    };

    // 2. Détermination du Workspace de destination
    let active_domain = settings["target_domain"]
        .as_str()
        .unwrap_or(&config.mount_points.modeling.domain);

    let active_db = settings["target_db"]
        .as_str()
        .unwrap_or(&config.mount_points.modeling.db);

    let target_manager = CollectionsManager::new(&ctx.db, active_domain, active_db);

    // 3. Traitement itératif des documents (Validation DDL + JSON-LD via la Forteresse)
    for mut doc in docs {
        let doc_id = match doc
            .get("_id")
            .or_else(|| doc.get("id"))
            .and_then(|v| v.as_str())
        {
            Some(id) => id.to_string(),
            None => {
                user_warn!(
                    "WARN_ARTIFACT_IGNORED",
                    json_value!({ "reason": "ID manquant", "doc": doc })
                );
                continue;
            }
        };

        let name = doc["name"].as_str().unwrap_or("Unnamed").to_string();
        let element_type = doc
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("UnknownElement")
            .to_string();

        let route_opt = mapping_doc
            .get("mappings")
            .and_then(|m| m.get(&element_type));

        let layer = match route_opt.and_then(|r| r.get("layer").and_then(|l| l.as_str())) {
            Some(l) => l.to_string(),
            None => doc["layer"].as_str().unwrap_or("unknown").to_string(),
        };

        let collection = match route_opt.and_then(|r| r.get("collection").and_then(|c| c.as_str()))
        {
            Some(c) => c.to_string(),
            None => "elements".to_string(),
        };

        if let Some(obj) = doc.as_object_mut() {
            if !obj.contains_key("_id") {
                obj.insert("_id".to_string(), json_value!(doc_id.clone()));
            }
        }

        // 🎯 STRICT : Utilisation de insert_with_schema pour forcer le passage par la "Forteresse"
        match target_manager.insert_with_schema(&collection, doc).await {
            Ok(_) => {
                artifacts.push(CreatedArtifact {
                    id: doc_id.clone(),
                    name,
                    layer: layer.to_uppercase(),
                    element_type,
                    path: format!("ref:{}:id:{}", collection, doc_id),
                });
            }
            Err(e) => {
                user_warn!(
                    "WARN_ARTIFACT_SAVE_FAILED",
                    json_value!({
                        "id": doc_id,
                        "collection": collection,
                        "error": e.to_string()
                    })
                );
            }
        }
    }

    Ok(artifacts)
}

/// Interroge le Knowledge Graph système
pub async fn query_knowledge_graph(
    ctx: &AgentContext,
    reference: &str,
    as_rdf: bool,
) -> RaiseResult<JsonValue> {
    let config = AppConfig::get();

    let target_domain = &config.mount_points.system.domain;
    let target_db = &config.mount_points.system.db;

    let tool = QueryDbTool::new(
        ctx.db.clone(),
        target_domain.to_string(),
        target_db.to_string(),
    );
    let call = McpToolCall::new(
        "query_db",
        json_value!({ "reference": reference, "as_rdf": as_rdf }),
    );

    let result = tool.execute(call).await;

    if result.is_error {
        // 🎯 FIX : Utilisation directe de la macro divergente
        raise_error!(
            "ERR_AGENT_QUERY_DB_FAIL",
            error = result
                .content
                .as_str()
                .unwrap_or("Erreur de requête KG inconnue"),
            context = json_value!({ "reference": reference })
        );
    }

    match result.content.get("data") {
        Some(data) => Ok(data.clone()),
        None => {
            // 🎯 FIX : Divergence pure
            raise_error!(
                "ERR_AGENT_KG_INVALID_PAYLOAD",
                error = "La réponse de la base ne contient pas d'objet 'data'.",
                context = json_value!({ "reference": reference })
            );
        }
    }
}

/// Charge l'historique d'une session agent depuis la base système
pub async fn load_session(ctx: &AgentContext) -> RaiseResult<AgentSession> {
    let config = AppConfig::get();
    let target_domain = &config.mount_points.system.domain;
    let target_db = &config.mount_points.system.db;

    let handle_slug = format!("{}-{}", ctx.session_id, ctx.agent_id)
        .replace(":", "-")
        .replace("_", "-")
        .to_lowercase();

    let tool = QueryDbTool::new(
        ctx.db.clone(),
        target_domain.to_string(),
        target_db.to_string(),
    );
    let call = McpToolCall::new(
        "query_db",
        json_value!({ "reference": format!("ref:session_agents:handle:{}", handle_slug), "as_rdf": false }),
    );

    let result = tool.execute(call).await;
    let mut session = AgentSession::new(&ctx.session_id, &ctx.agent_id);

    if !result.is_error {
        let doc = &result.content["data"];
        if let Some(msgs_array) = doc.get("messages").and_then(|m| m.as_array()) {
            if let Ok(msgs) = json::deserialize_from_value(json_value!(msgs_array)) {
                session.messages = msgs;
            }
        }
        if let Some(summary) = doc.get("summary").and_then(|s| s.as_str()) {
            session.summary = Some(summary.to_string());
        }
    } else {
        if let Err(e) = Box::pin(save_session(ctx, &session)).await {
            user_warn!(
                "WARN_SESSION_INIT_SAVE_FAILED",
                json_value!({"err": e.to_string()})
            );
        }
    }

    Ok(session)
}

/// Sauvegarde l'état actuel de la session agent
pub async fn save_session(ctx: &AgentContext, session: &AgentSession) -> RaiseResult<()> {
    let config = AppConfig::get();
    let target_domain = &config.mount_points.system.domain;
    let target_db = &config.mount_points.system.db;

    let manager = CollectionsManager::new(&ctx.db, target_domain, target_db);

    let handle_slug = format!("{}-{}", ctx.session_id, ctx.agent_id)
        .replace(":", "-")
        .replace("_", "-")
        .to_lowercase();

    let session_doc = json_value!({
        "handle": handle_slug,
        "session_id": ctx.session_id,
        "agent_id": ctx.agent_id,
        "status": "active",
        "messages": session.messages,
        "summary": session.summary,
        "updated_at": UtcClock::now().to_rfc3339()
    });

    if let Err(e) = manager.upsert_document("session_agents", session_doc).await {
        // 🎯 FIX : Divergence pure sans enveloppe
        raise_error!(
            "ERR_SESSION_DB_SAVE_FAIL",
            error = e,
            context = json_value!({ "session_handle": handle_slug })
        );
    }

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
        let input = r#"
Voici l'analyse demandée :

```json
{"status": "ok"}
"#;
        assert_eq!(extract_json_from_llm(input), "{\"status\": \"ok\"}");
    }
}
