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
pub struct HardwareAgent;

impl HardwareAgent {
    pub fn new() -> Self {
        Self {}
    }

    // Méthode publique pour être testable ou utilisée ailleurs
    pub fn determine_category(&self, name: &str, element_type: &str) -> &'static str {
        let keywords = format!("{} {}", name, element_type).to_lowercase();
        if keywords.contains("fpga") || keywords.contains("asic") || keywords.contains("pcb") {
            "Electronics"
        } else {
            "Infrastructure"
        }
    }

    async fn enrich_physical_node(
        &self,
        ctx: &AgentContext,
        name: &str,
        element_type: &str,
    ) -> Result<serde_json::Value> {
        let category = self.determine_category(name, element_type);
        let instruction = if category == "Electronics" {
            "Contexte: Design Électronique."
        } else {
            "Contexte: Infrastructure IT."
        };

        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[COMPOSANTS]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Matériel. Génère JSON.";
        let user_prompt = format!(
            "Crée Noeud PA.\nNom: {}\nType: {}\n{}\n{}\nJSON: {{ \"name\": \"str\", \"specs\": {{}} }}",
            name, element_type, instruction, nlp_hint
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, &user_prompt)
            .await
            .map_err(|e| anyhow!("Erreur LLM Hardware: {}", e))?;

        let clean_json = extract_json_from_llm(&response);
        let mut data: serde_json::Value =
            serde_json::from_str(&clean_json).unwrap_or(json!({ "name": name }));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("PA");
        data["type"] = json!("PhysicalNode");
        data["nature"] = json!(category);
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        Ok(data)
    }
}

#[async_trait]
impl Agent for HardwareAgent {
    fn id(&self) -> &'static str {
        "hardware_architect"
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
            } if layer == "PA" => {
                let doc = self.enrich_physical_node(ctx, name, element_type).await?;
                let nature = doc["nature"].as_str().unwrap_or("Hardware").to_string();

                let artifact = save_artifact(ctx, "pa", "physical_nodes", &doc)?;

                Ok(Some(AgentResult {
                    message: format!("Noeud physique **{}** ({}) provisionné.", name, nature),
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
    fn test_category_detection() {
        let agent = HardwareAgent::new();
        assert_eq!(agent.determine_category("Carte Mère", "PCB"), "Electronics");
        assert_eq!(
            agent.determine_category("Serveur", "Rack"),
            "Infrastructure"
        );
    }
}
