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
pub struct DataAgent;

impl DataAgent {
    pub fn new() -> Self {
        Self {}
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
            .map_err(|e| anyhow!("LLM Err: {}", e))?;

        let clean = extract_json_from_llm(&response);
        let mut doc: serde_json::Value = serde_json::from_str(&clean).unwrap_or(json!({}));

        // --- BLINDAGE TOTAL ---
        doc["name"] = json!(original_name);
        doc["id"] = json!(Uuid::new_v4().to_string());
        doc["layer"] = json!("DATA");
        doc["type"] = json!(doc_type);
        doc["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        if doc_type == "Class" && doc.get("attributes").is_none() {
            doc["attributes"] = json!([]);
        }
        Ok(doc)
    }

    async fn enrich_class(&self, ctx: &AgentContext, name: &str) -> Result<serde_json::Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Attributs potentiels :\n");
            for e in entities {
                nlp_hint.push_str(&format!("- {}\n", e.text));
            }
        }
        let sys = "Tu es Data Architect. JSON Strict.";
        let user = format!(
            "Nom: {}\n{}\nJSON: {{ \"name\": \"{}\", \"attributes\": [] }}",
            name, nlp_hint, name
        );
        self.call_llm(ctx, sys, &user, "Class", name).await
    }
}

#[async_trait]
impl Agent for DataAgent {
    fn id(&self) -> &'static str {
        "data_architect"
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
            } if layer == "DATA" => {
                let (doc, collection) = match element_type.to_lowercase().as_str() {
                    "class" | "classe" => (self.enrich_class(ctx, name).await?, "classes"),
                    _ => (self.enrich_class(ctx, name).await?, "classes"),
                };

                let artifact = save_artifact(ctx, "data", collection, &doc)?;

                Ok(Some(AgentResult {
                    message: format!("Donnée **{}** ({}) définie.", name, element_type),
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
    fn test_data_agent_sanity() {
        assert_eq!(DataAgent::new().id(), "data_architect");
    }
}
