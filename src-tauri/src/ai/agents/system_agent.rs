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
pub struct SystemAgent;

impl SystemAgent {
    pub fn new() -> Self {
        Self
    }

    async fn enrich_sa_element(
        &self,
        ctx: &AgentContext,
        name: &str,
        element_type: &str,
    ) -> Result<serde_json::Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[VOCABULAIRE]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Système (Arcadia). JSON Strict.";
        let user_prompt = format!(
            "Crée un élément SA.\nType: {}\nNom: {}\n{}\nJSON Attendu: {{ \"name\": \"str\", \"description\": \"str\" }}",
            element_type, name, nlp_hint
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, &user_prompt)
            .await
            .map_err(|e| anyhow!("LLM Error: {}", e))?;

        let clean_json = extract_json_from_llm(&response);
        let mut data: serde_json::Value = serde_json::from_str(&clean_json)
            .unwrap_or(json!({ "name": name, "description": "Auto-generated" }));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("SA");
        data["type"] = json!(format!("System{}", element_type));
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        Ok(data)
    }
}

#[async_trait]
impl Agent for SystemAgent {
    fn id(&self) -> &'static str {
        "system_architect"
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
            } if layer == "SA" => {
                let doc = self.enrich_sa_element(ctx, name, element_type).await?;

                let collection = match element_type.to_lowercase().as_str() {
                    "function" | "fonction" => "functions",
                    "actor" | "acteur" => "actors",
                    "component" | "composant" | "system" => "components",
                    "capability" | "capacité" => "capabilities",
                    _ => "functions",
                };

                let artifact = save_artifact(ctx, "sa", collection, &doc)?;

                Ok(Some(AgentResult {
                    message: format!("J'ai défini l'élément **{}** dans l'analyse système.", name),
                    artifacts: vec![artifact],
                }))
            }
            EngineeringIntent::CreateRelationship { .. } => Ok(Some(AgentResult::text(
                "⚠️ SystemAgent: Les relations sont en cours de migration.".to_string(),
            ))),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_id() {
        let agent = SystemAgent::new();
        assert_eq!(agent.id(), "system_architect");
    }
}
