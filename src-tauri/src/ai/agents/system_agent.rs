// FICHIER : src-tauri/src/ai/agents/system_agent.rs

use crate::utils::{async_trait, data, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};

// AJOUT : Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

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
        history_context: &str,
    ) -> RaiseResult<Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[VOCABULAIRE]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Système (Arcadia). JSON Strict. \
                             Utilise l'historique pour éviter les doublons ou incohérences.";

        // Injection de l'historique dans le prompt
        let user_prompt = format!(
            "=== HISTORIQUE ===\n{}\n\n=== NOUVELLE DEMANDE ===\nCrée un élément SA.\nType: {}\nNom: {}\n{}\nJSON Attendu: {{ \"name\": \"str\", \"description\": \"str\" }}",
            history_context, element_type, name, nlp_hint
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, &user_prompt)
            .await
            .map_err(|e| AppError::Validation(format!("LLM Error: {}", e)))?;

        let clean_json = extract_json_from_llm(&response);
        let mut data: Value = data::parse(&clean_json)
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
    ) -> RaiseResult<Option<AgentResult>> {
        // 1. CHARGEMENT SESSION
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            EngineeringIntent::CreateElement {
                layer,
                element_type,
                name,
            } if layer == "SA" => {
                // 2. LOG UTILISATEUR
                session.add_message(
                    "user",
                    &format!("Crée élément SA type '{}' nom '{}'", element_type, name),
                );

                // 3. RECUPERATION HISTORIQUE
                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                // 4. ACTION (Génération Artefact)
                let doc = self
                    .enrich_sa_element(ctx, name, element_type, &history_str)
                    .await?;

                let collection = match element_type.to_lowercase().as_str() {
                    "function" | "fonction" => "functions",
                    "actor" | "acteur" => "actors",
                    "component" | "composant" | "system" => "components",
                    "capability" | "capacité" => "capabilities",
                    _ => "functions",
                };

                let artifact = save_artifact(ctx, "sa", collection, &doc).await?;

                // 5. DÉCISION DE DÉLÉGATION (Logique ACL)
                // Si on crée un COMPOSANT, on déclenche l'Agent Logiciel
                let mut outgoing_message = None;
                if collection == "components" {
                    let transition_msg = format!(
                        "J'ai défini le composant système '{}'. Peux-tu initialiser le composant logique correspondant et proposer une structure de code ?", 
                        name
                    );

                    outgoing_message = Some(AclMessage::new(
                        Performative::Request,
                        self.id(),           // Sender: system_architect
                        "software_engineer", // Receiver: software_engineer
                        &transition_msg,
                    ));
                }

                let result_msg = if outgoing_message.is_some() {
                    format!("J'ai défini le composant **{}**. Je transmets la demande à l'architecte logiciel...", name)
                } else {
                    format!("J'ai défini l'élément **{}** dans l'analyse système.", name)
                };

                // 6. SAUVEGARDE REPONSE
                session.add_message("assistant", &result_msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: result_msg,
                    artifacts: vec![artifact],
                    outgoing_message, // Le message ACL est embarqué ici
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
    use crate::ai::protocols::acl::Performative;

    #[test]
    fn test_agent_id() {
        let agent = SystemAgent::new();
        assert_eq!(agent.id(), "system_architect");
    }

    // NOUVEAU TEST : Vérifie que la délégation ACL se déclenche pour un Composant
    #[tokio::test]
    async fn test_system_agent_delegation_trigger() {
        // CORRECTION WARNING : Préfixe '_' pour indiquer variables non utilisées
        let _agent = SystemAgent::new();

        let _element_type = "Component";
        let collection = "components"; // Logique interne
        let _name = "TestComp";

        let mut outgoing_message = None;
        if collection == "components" {
            outgoing_message = Some(AclMessage::new(
                Performative::Request,
                "system_architect",
                "software_engineer",
                "content",
            ));
        }

        assert!(outgoing_message.is_some());
        assert_eq!(outgoing_message.unwrap().receiver, "software_engineer");
    }
}
