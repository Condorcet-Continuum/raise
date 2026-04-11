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

/// Sauvegarde un artefact métier dynamiquement via l'Ontologie (Knowledge Graph)
pub async fn save_artifact(ctx: &AgentContext, doc: &JsonValue) -> RaiseResult<CreatedArtifact> {
    // 🎯 Zéro Dette : Match explicite sur l'ID pour éviter les corruptions
    let doc_id = match doc
        .get("_id")
        .or_else(|| doc.get("id"))
        .and_then(|v| v.as_str())
    {
        Some(id) => id.to_string(),
        None => {
            raise_error!(
                "ERR_ARTIFACT_ID_INVALID",
                error = "L'artefact produit par l'IA n'a pas d'identifiant valide.",
                context = json_value!({ "doc": doc })
            );
        }
    };

    let name = doc["name"].as_str().unwrap_or("Unnamed").to_string();
    let element_type = doc
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("UnknownElement")
        .to_string();

    // 🎯 Résolution du mapping via le Knowledge Graph
    let mapping_doc =
        match query_knowledge_graph(ctx, "ref:configs:handle:ontological_mapping", false).await {
            Ok(d) => d,
            Err(_) => {
                raise_error!(
                    "ERR_MISSING_ONTOLOGY_MAPPING",
                    error = "Le document de configuration 'ontological_mapping' est introuvable."
                );
            }
        };

    let route = &mapping_doc["mappings"][&element_type];
    let layer = route["layer"]
        .as_str()
        .unwrap_or_else(|| doc["layer"].as_str().unwrap_or("unknown"));
    let collection = route["collection"].as_str().unwrap_or("elements");

    let config = AppConfig::get();

    // 🎯 FIX MOUNT POINTS : Utilisation du domaine système via les points de montage
    let sys_mgr = CollectionsManager::new(
        &ctx.db,
        &config.mount_points.system.domain,
        &config.mount_points.system.db,
    );

    let settings = AppConfig::get_component_settings(&sys_mgr, "ai_agents")
        .await
        .unwrap_or(json_value!({}));

    // On détermine la destination (par défaut le workspace actif)
    let active_domain = settings["target_domain"]
        .as_str()
        .unwrap_or(&config.mount_points.modeling.domain);

    let active_db = settings["target_db"]
        .as_str()
        .unwrap_or(&config.mount_points.modeling.db);

    let target_manager = CollectionsManager::new(&ctx.db, active_domain, active_db);

    let mut final_doc = doc.clone();
    if let Some(obj) = final_doc.as_object_mut() {
        if !obj.contains_key("_id") {
            obj.insert("_id".to_string(), json_value!(doc_id.clone()));
        }
    }

    target_manager
        .upsert_document(collection, final_doc)
        .await?;

    Ok(CreatedArtifact {
        id: doc_id.clone(),
        name,
        layer: layer.to_uppercase(),
        element_type,
        path: format!("ref:{}:id:{}", collection, doc_id),
    })
}

/// Interroge le Knowledge Graph système
pub async fn query_knowledge_graph(
    ctx: &AgentContext,
    reference: &str,
    as_rdf: bool,
) -> RaiseResult<JsonValue> {
    let config = AppConfig::get();

    // 🎯 FIX MOUNT POINTS : Résolution via point de montage système
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
        raise_error!(
            "ERR_AGENT_QUERY_DB_FAIL",
            error = result
                .content
                .as_str()
                .unwrap_or("Erreur de requête KG inconnue"),
            context = json_value!({ "reference": reference })
        );
    }

    Ok(result.content["data"].clone())
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
        if let Some(msgs_array) = doc["messages"].as_array() {
            if let Ok(msgs) = json::deserialize_from_value(json_value!(msgs_array)) {
                session.messages = msgs;
            }
        }
        if let Some(summary) = doc["summary"].as_str() {
            session.summary = Some(summary.to_string());
        }
    } else {
        // Sauvegarde initiale si inexistante
        let _ = Box::pin(save_session(ctx, &session)).await;
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

    // Récupération de l'ID existant pour éviter les doublons
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

    let mut session_doc = json_value!({
        "handle": handle_slug,
        "session_id": ctx.session_id,
        "agent_id": ctx.agent_id,
        "status": "active",
        "messages": session.messages,
        "summary": session.summary,
        "updated_at": UtcClock::now().to_rfc3339()
    });

    if !result.is_error {
        if let Some(id) = result.content["data"].get("_id") {
            if let Some(obj) = session_doc.as_object_mut() {
                obj.insert("_id".to_string(), id.clone());
            }
        }
    }

    manager
        .upsert_document("session_agents", session_doc)
        .await?;
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
