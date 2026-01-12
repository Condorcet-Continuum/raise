use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, save_artifact};
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

        // Defaults
        if doc_type == "Requirement" && doc.get("reqId").is_none() {
            doc["reqId"] = json!("REQ-AUTO");
        }

        Ok(doc)
    }

    async fn enrich_requirement(
        &self,
        ctx: &AgentContext,
        name: &str,
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
            "Exigence: \"{}\"\n{}\nJSON: {{ \"statement\": \"str\", \"reqId\": \"REQ-01\" }}",
            name, nlp_hint
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
        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "TRANSVERSE" => {
                let et_lower = element_type.to_lowercase();

                let (doc, sub_folder) = match et_lower.as_str() {
                    "requirement" | "exigence" => {
                        (self.enrich_requirement(ctx, name).await?, "requirements")
                    }
                    _ => (self.enrich_requirement(ctx, name).await?, "requirements"),
                };

                let artifact = save_artifact(ctx, "transverse", sub_folder, &doc)?;

                Ok(Some(AgentResult {
                    message: format!("Élément Transverse **{}** ({}) créé.", name, element_type),
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
    fn test_transverse_id() {
        assert_eq!(TransverseAgent::new().id(), "quality_manager");
    }
}
