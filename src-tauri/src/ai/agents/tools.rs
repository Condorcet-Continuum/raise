// FICHIER : src-tauri/src/ai/agents/tools.rs

use super::{AgentContext, AgentSession, CreatedArtifact};
use crate::ai::protocols::mcp::{McpTool, McpToolCall};
use crate::ai::tools::QueryDbTool;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*;

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

/// Sauvegarde un artefact métier dynamiquement via l'Ontologie
pub async fn save_artifact(ctx: &AgentContext, doc: &JsonValue) -> RaiseResult<CreatedArtifact> {
    let Some(doc_id_ref) = doc
        .get("_id")
        .or_else(|| doc.get("id"))
        .and_then(|v| v.as_str())
    else {
        raise_error!(
            "ERR_ARTIFACT_ID_INVALID",
            error = "L'artefact n'a pas d'ID valide",
            context = json_value!({ "doc": doc })
        );
    };
    let doc_id = doc_id_ref.to_string();
    let name = doc["name"].as_str().unwrap_or("Unnamed").to_string();
    let element_type = doc
        .get("type")
        .and_then(|t| t.as_str())
        .unwrap_or("UnknownElement")
        .to_string();

    let mapping_doc =
        match query_knowledge_graph(ctx, "ref:configs:handle:ontological_mapping", false).await {
            Ok(d) => d,
            Err(_) => raise_error!(
                "ERR_MISSING_ONTOLOGY_MAPPING",
                error =
                    "Le document de configuration 'ontological_mapping' est introuvable en base."
            ),
        };

    let route = &mapping_doc["mappings"][&element_type];
    let layer = route["layer"]
        .as_str()
        .unwrap_or_else(|| doc["layer"].as_str().unwrap_or("unknown"));
    let collection = route["collection"].as_str().unwrap_or("elements");

    let config = AppConfig::get();
    let sys_mgr = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);
    let settings = AppConfig::get_component_settings(&sys_mgr, "ai_agents")
        .await
        .unwrap_or(json_value!({}));

    let active_domain = settings["target_domain"].as_str().unwrap_or("un2");
    let active_db = layer.to_lowercase();

    let target_manager = CollectionsManager::new(&ctx.db, active_domain, &active_db);

    let mut final_doc = doc.clone();
    if let Some(obj) = final_doc.as_object_mut() {
        if !obj.contains_key("_id") {
            obj.insert("_id".to_string(), json_value!(doc_id.clone()));
        }
    }

    target_manager
        .upsert_document(collection, final_doc)
        .await?;

    let virtual_path = format!("ref:{}:id:{}", collection, doc_id);

    Ok(CreatedArtifact {
        id: doc_id,
        name,
        layer: layer.to_uppercase(),
        element_type,
        path: virtual_path,
    })
}

pub async fn query_knowledge_graph(
    ctx: &AgentContext,
    reference: &str,
    as_rdf: bool,
) -> RaiseResult<JsonValue> {
    let config = AppConfig::get();
    let sys_mgr = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);
    let settings = AppConfig::get_component_settings(&sys_mgr, "ai_agents")
        .await
        .unwrap_or(json_value!({}));

    let target_domain = settings["system_domain"]
        .as_str()
        .unwrap_or(&config.system_domain)
        .to_string();
    let target_db = settings["system_db"]
        .as_str()
        .unwrap_or(&config.system_db)
        .to_string();

    let tool = QueryDbTool::new(ctx.db.clone(), target_domain, target_db);
    let call = McpToolCall::new(
        "query_db",
        json_value!({ "reference": reference, "as_rdf": as_rdf }),
    );
    let result = tool.execute(call).await;

    if result.is_error {
        raise_error!(
            "ERR_AGENT_QUERY_DB_FAIL",
            error = result.content.as_str().unwrap_or("Erreur inconnue"),
            context = json_value!({ "target_reference": reference })
        );
    }
    Ok(result.content["data"].clone())
}

pub async fn find_element_by_name(ctx: &AgentContext, name: &str) -> Option<JsonValue> {
    let config = AppConfig::get();
    let sys_mgr = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);
    let settings = AppConfig::get_component_settings(&sys_mgr, "ai_agents")
        .await
        .unwrap_or(json_value!({}));
    let active_domain = settings["target_domain"]
        .as_str()
        .unwrap_or("un2")
        .to_string();

    let mapping_doc = query_knowledge_graph(ctx, "ref:configs:handle:ontological_mapping", false)
        .await
        .ok()?;
    let search_spaces = mapping_doc["search_spaces"].as_array()?;

    for space in search_spaces {
        let layer_db = space["layer"].as_str().unwrap_or("raise");
        let col = space["collection"].as_str().unwrap_or("");

        let tool = QueryDbTool::new(
            ctx.db.clone(),
            active_domain.clone(),
            layer_db.to_lowercase(),
        );
        let reference = format!("ref:{}:name:{}", col, name);

        let call = McpToolCall::new(
            "query_db",
            json_value!({ "reference": reference, "as_rdf": false }),
        );
        let result = tool.execute(call).await;
        if !result.is_error {
            return Some(result.content["data"].clone());
        }
    }
    None
}

pub async fn load_session(ctx: &AgentContext) -> RaiseResult<AgentSession> {
    let config = AppConfig::get();
    let sys_mgr = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);
    let settings = AppConfig::get_component_settings(&sys_mgr, "ai_agents")
        .await
        .unwrap_or(json_value!({}));
    let target_domain = settings["system_domain"]
        .as_str()
        .unwrap_or(&config.system_domain);
    let target_db = settings["system_db"].as_str().unwrap_or(&config.system_db);

    // 🎯 PRODUCTION STRICTE : On fait confiance à l'infrastructure.
    // Plus de `create_collection` sauvage au runtime ! L'upsert / query_db s'occupera du reste.

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
        let doc_value = &result.content["data"];

        if let Some(msgs_array) = doc_value["messages"].as_array() {
            if let Ok(msgs) = json::deserialize_from_value(json_value!(msgs_array)) {
                session.messages = msgs;
            }
        }
        if let Some(summary) = doc_value["summary"].as_str() {
            session.summary = Some(summary.to_string());
        }
    } else {
        let _ = Box::pin(save_session(ctx, &session)).await;
    }

    Ok(session)
}

pub async fn save_session(ctx: &AgentContext, session: &AgentSession) -> RaiseResult<()> {
    let config = AppConfig::get();
    let sys_mgr = CollectionsManager::new(&ctx.db, &config.system_domain, &config.system_db);
    let settings = AppConfig::get_component_settings(&sys_mgr, "ai_agents")
        .await
        .unwrap_or(json_value!({}));
    let target_domain = settings["system_domain"]
        .as_str()
        .unwrap_or(&config.system_domain);
    let target_db = settings["system_db"].as_str().unwrap_or(&config.system_db);
    let manager = CollectionsManager::new(&ctx.db, target_domain, target_db);

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

    let mut session_doc = json_value!({
        "handle": handle_slug,
        "session_id": ctx.session_id,
        "agent_id": ctx.agent_id,
        "status": "idle",
        "messages": session.messages,
        "summary": session.summary,
        "memory_state": { "thread_id": handle_slug, "turns_count": session.messages.len() },
        "metrics": { "tokens_prompt": 0, "tokens_completion": 0, "total_compute_time_ms": 0 }
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
