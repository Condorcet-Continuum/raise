use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, save_artifact};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;
use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

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
    ) -> Result<serde_json::Value> {
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
            "Item: {}. Type: {}. {}\nJSON: {{ \"name\": \"str\", \"partNumber\": \"PN-XXX\" }}",
            name, raw_type, nlp_hint
        );

        let res = ctx
            .llm
            .ask(LlmBackend::LocalLlama, sys, &user)
            .await
            .map_err(|e| anyhow!("{}", e))?;

        let clean = extract_json_from_llm(&res);
        let mut data: serde_json::Value =
            serde_json::from_str(&clean).unwrap_or(json!({"name": name, "partNumber": "UNK"}));

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
    ) -> Result<Option<AgentResult>> {
        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "EPBS" => {
                let doc = self.enrich_item(ctx, name, element_type).await?;
                let artifact = save_artifact(ctx, "epbs", "configuration_items", &doc)?;

                Ok(Some(AgentResult {
                    message: format!("Article **{}** (EPBS) créé.", name),
                    artifacts: vec![artifact],
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
