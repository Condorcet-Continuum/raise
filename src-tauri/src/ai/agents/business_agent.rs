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
pub struct BusinessAgent;

impl BusinessAgent {
    pub fn new() -> Self {
        Self {}
    }

    async fn analyze_business_need(
        &self,
        ctx: &AgentContext,
        domain: &str,
        description: &str,
    ) -> Result<serde_json::Value> {
        let entities = entity_extractor::extract_entities(description);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Acteurs potentiels :\n");
            for entity in entities {
                nlp_hint.push_str(&format!("- {}\n", entity.text));
            }
        }

        let system_prompt =
            "Tu es un Business Analyst Senior. Extrais Capacité et Acteurs en JSON.";
        let user_prompt = format!(
            "Domaine: {}\nBesoin: {}\n{}\nJSON: {{ \"capability\": {{ \"name\": \"str\", \"description\": \"str\" }}, \"actors\": [ {{ \"name\": \"str\" }} ] }}",
            domain, description, nlp_hint
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, &user_prompt)
            .await
            .map_err(|e| anyhow!("Erreur LLM Business: {}", e))?;

        let clean = extract_json_from_llm(&response);
        Ok(serde_json::from_str(&clean).unwrap_or(json!({})))
    }
}

#[async_trait]
impl Agent for BusinessAgent {
    fn id(&self) -> &'static str {
        "business_analyst"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> Result<Option<AgentResult>> {
        if let EngineeringIntent::DefineBusinessUseCase {
            domain,
            process_name,
            description,
        } = intent
        {
            // CORRECTION : Suppression de 'mut' ici
            let analysis = self
                .analyze_business_need(ctx, domain, description)
                .await
                .unwrap_or(json!({}));

            let cap_desc = analysis["capability"]["description"]
                .as_str()
                .unwrap_or(description)
                .to_string();

            // 1. Capacité
            let cap_id = Uuid::new_v4().to_string();
            let cap_doc = json!({
                "id": cap_id,
                "name": process_name,
                "description": cap_desc,
                "layer": "OA",
                "type": "OperationalCapability",
                "domain": domain,
                "createdAt": chrono::Utc::now().to_rfc3339()
            });

            let mut artifacts = vec![];
            artifacts.push(save_artifact(ctx, "oa", "capabilities", &cap_doc)?);

            // 2. Acteurs
            if let Some(actors) = analysis["actors"].as_array() {
                for actor in actors {
                    let actor_name = actor["name"].as_str().unwrap_or("UnknownActor");
                    let act_id = Uuid::new_v4().to_string();
                    let act_doc = json!({
                        "id": act_id,
                        "name": actor_name,
                        "layer": "OA",
                        "type": "OperationalActor",
                        "createdAt": chrono::Utc::now().to_rfc3339()
                    });

                    artifacts.push(save_artifact(ctx, "oa", "actors", &act_doc)?);
                }
            }

            return Ok(Some(AgentResult {
                message: format!("Analyse **{}** terminée.", process_name),
                artifacts,
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_business_id() {
        assert_eq!(BusinessAgent::new().id(), "business_analyst");
    }
}
