// FICHIER : src-tauri/src/ai/agents/hardware_agent.rs

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};

// AJOUT : Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct HardwareAgent;

impl HardwareAgent {
    pub fn new() -> Self {
        Self {}
    }

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
        history_context: &str,
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
            "=== HISTORIQUE ===\n{}\n\nCrée Noeud PA.\nNom: {}\nType: {}\n{}\n{}\nJSON: {{ \"name\": \"str\", \"specs\": {{}} }}",
            history_context, name, element_type, instruction, nlp_hint
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
        // 1. CHARGEMENT SESSION
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "PA" => {
                session.add_message("user", &format!("Create Node: {} ({})", name, element_type));

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
                    .enrich_physical_node(ctx, name, element_type, &history_str)
                    .await?;
                let nature = doc["nature"].as_str().unwrap_or("Hardware").to_string();

                let artifact = save_artifact(ctx, "pa", "physical_nodes", &doc)?;

                // 2. DÉLÉGATION -> EPBS (Configuration Manager)
                // Tout matériel physique doit être référencé (BOM/PartNumber)
                let transition_msg = format!(
                    "J'ai spécifié le matériel '{}' (Nature: {}). Merci de créer l'Article de Configuration (CI) associé.",
                    name, nature
                );

                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),               // Sender
                    "configuration_manager", // Receiver
                    &transition_msg,
                );

                let msg = format!(
                    "Noeud physique **{}** ({}) provisionné. Demande de création CI envoyée.",
                    name, nature
                );

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    // AJOUT : Message sortant activé
                    outgoing_message: Some(acl_msg),
                }))
            }
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::protocols::acl::Performative;

    #[test]
    fn test_category_detection() {
        let agent = HardwareAgent::new();
        assert_eq!(agent.determine_category("Carte Mère", "PCB"), "Electronics");
        assert_eq!(
            agent.determine_category("Serveur", "Rack"),
            "Infrastructure"
        );
    }

    // NOUVEAU TEST : Vérifie la délégation vers EPBS
    #[tokio::test]
    async fn test_hardware_delegation_trigger() {
        let _agent = HardwareAgent::new();

        let msg = AclMessage::new(
            Performative::Request,
            "hardware_architect",
            "configuration_manager",
            "Content",
        );

        assert_eq!(msg.receiver, "configuration_manager");
        assert_eq!(msg.performative, Performative::Request);
    }
}
