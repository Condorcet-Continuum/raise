// FICHIER : src-tauri/src/ai/agents/transverse_agent.rs

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct TransverseAgent;

impl TransverseAgent {
    pub fn new() -> Self {
        Self
    }

    async fn call_llm(
        &self,
        ctx: &AgentContext,
        sys: &str,
        user: &str,
        doc_type: &str,
        original_name: &str,
    ) -> Result<serde_json::Value> {
        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, sys, user)
            .await
            .map_err(|e| anyhow!("LLM Transverse: {}", e))?;

        let clean = extract_json_from_llm(&response);
        let mut doc: serde_json::Value = serde_json::from_str(&clean).unwrap_or(json!({}));

        // --- BLINDAGE ---
        doc["name"] = json!(original_name);
        doc["id"] = json!(Uuid::new_v4().to_string());
        doc["layer"] = json!("TRANSVERSE");
        doc["type"] = json!(doc_type);
        doc["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        if doc_type == "Requirement" && doc.get("reqId").is_none() {
            doc["reqId"] = json!("REQ-AUTO");
        }

        Ok(doc)
    }

    async fn enrich_requirement(
        &self,
        ctx: &AgentContext,
        name: &str,
        history_context: &str, // AJOUT : Contexte
    ) -> Result<serde_json::Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Concerne :\n");
            for e in entities {
                nlp_hint.push_str(&format!("- {}\n", e.text));
            }
        }
        let sys = "RÔLE: Ingénieur Exigences. JSON Strict.";
        let user = format!(
            "=== HISTORIQUE ===\n{}\n\nExigence: \"{}\"\n{}\nJSON: {{ \"statement\": \"str\", \"reqId\": \"REQ-01\" }}",
            history_context, name, nlp_hint
        );
        self.call_llm(ctx, sys, &user, "Requirement", name).await
    }
}

#[async_trait]
impl Agent for TransverseAgent {
    fn id(&self) -> &'static str {
        "quality_manager"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> Result<Option<AgentResult>> {
        // 1. CHARGEMENT SESSION
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "TRANSVERSE" => {
                session.add_message(
                    "user",
                    &format!("Create Transverse: {} ({})", name, element_type),
                );

                // Calcul Historique
                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let et_lower = element_type.to_lowercase();
                let (doc, sub_folder) = match et_lower.as_str() {
                    "requirement" | "exigence" => (
                        self.enrich_requirement(ctx, name, &history_str).await?,
                        "requirements",
                    ),
                    _ => (
                        self.enrich_requirement(ctx, name, &history_str).await?,
                        "requirements",
                    ),
                };

                let artifact = save_artifact(ctx, "transverse", sub_folder, &doc)?;
                let msg = format!("Élément Transverse **{}** ({}) créé.", name, element_type);

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    // CORRECTION : Champ ajouté
                    outgoing_message: None,
                }))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_transverse_id() {
        assert_eq!(TransverseAgent::new().id(), "quality_manager");
    }
}
