// FICHIER : src-tauri/src/ai/agents/epbs_agent.rs

use crate::utils::{async_trait, data, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct EpbsAgent;

impl EpbsAgent {
    pub fn new() -> Self {
        Self {}
    }

    async fn enrich_item(
        &self,
        ctx: &AgentContext,
        name: &str,
        raw_type: &str,
        history_context: &str,
    ) -> RaiseResult<Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Ref:\n");
            for e in entities {
                nlp_hint.push_str(&format!("- {}\n", e.text));
            }
        }

        let sys = "Tu es Config Manager (EPBS). JSON Strict.";
        let user = format!(
            "=== HISTORIQUE ===\n{}\n\nItem: {}. Type: {}. {}\nJSON: {{ \"name\": \"str\", \"partNumber\": \"PN-XXX\" }}",
            history_context, name, raw_type, nlp_hint
        );

        let res = match ctx.llm.ask(LlmBackend::LocalLlama, sys, &user).await {
            Ok(val) => val,
            Err(e) => {
                // Cette macro exécute un 'return Err(...)'
                // Elle interrompt donc proprement la fonction de l'agent.
                raise_error!(
                    "ERR_AI_LLM_GENERATE",
                    error = e,
                    context = serde_json::json!({
                        "backend": "LocalLlama",
                        "user_prompt_len": user.len()
                    })
                );
            }
        };

        let clean = extract_json_from_llm(&res);
        let mut data: Value =
            data::parse(&clean).unwrap_or(json!({"name": name, "partNumber": "UNK"}));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("EPBS");
        data["type"] = json!("ConfigurationItem");
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());
        Ok(data)
    }
}

#[async_trait]
impl Agent for EpbsAgent {
    fn id(&self) -> &'static str {
        "configuration_manager"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> RaiseResult<Option<AgentResult>> {
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "EPBS" => {
                session.add_message("user", &format!("Create CI: {} ({})", name, element_type));

                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let doc = self
                    .enrich_item(ctx, name, element_type, &history_str)
                    .await?;
                let artifact = save_artifact(ctx, "epbs", "configuration_items", &doc).await?;
                let msg = format!("Article **{}** (EPBS) créé.", name);

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
    fn test_epbs_identity() {
        assert_eq!(EpbsAgent::new().id(), "configuration_manager");
    }
}
