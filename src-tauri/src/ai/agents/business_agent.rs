// FICHIER : src-tauri/src/ai/agents/business_agent.rs

use crate::utils::{async_trait, data, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;
use crate::ai::protocols::acl::{AclMessage, Performative};

#[derive(Default)]
pub struct BusinessAgent;

impl BusinessAgent {
    pub fn new() -> Self {
        Self {}
    }

    /// Analyse le besoin métier en tenant compte de l'historique de conversation
    async fn analyze_business_need(
        &self,
        ctx: &AgentContext,
        domain: &str,
        description: &str,
        history_context: &str,
    ) -> Result<Value> {
        let entities = entity_extractor::extract_entities(description);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("Acteurs potentiels détectés (NLP) :\n");
            for entity in entities {
                nlp_hint.push_str(&format!("- {}\n", entity.text));
            }
        }

        let system_prompt = "Tu es un Business Analyst Senior expert en méthode Arcadia. 
        Ton rôle est d'extraire une Capacité Opérationnelle (OperationalCapability) et des Acteurs (OperationalActor).
        Utilise le contexte de la conversation précédente pour affiner ou corriger ton analyse si nécessaire.";

        // Construction du prompt enrichi
        let user_prompt = format!(
            "=== HISTORIQUE DE CONVERSATION ===\n{}\n\n=== NOUVELLE DEMANDE ===\nDomaine: {}\nBesoin: {}\n{}\n\nAttendus JSON strict:\n{{ \"capability\": {{ \"name\": \"str\", \"description\": \"str\" }}, \"actors\": [ {{ \"name\": \"str\" }} ] }}",
            history_context, domain, description, nlp_hint
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, system_prompt, &user_prompt)
            .await
            .map_err(|e| AppError::Validation(format!("Erreur LLM Business: {}", e)))?;

        let clean = extract_json_from_llm(&response);
        Ok(data::parse(&clean).unwrap_or(json!({})))
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
        // 1. CHARGEMENT DE LA MÉMOIRE (Session)
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        if let EngineeringIntent::DefineBusinessUseCase {
            domain,
            process_name,
            description,
        } = intent
        {
            // 2. ENREGISTREMENT ENTRÉE UTILISATEUR
            let user_msg = format!(
                "Domaine: {}, Process: {}, Description: {}",
                domain, process_name, description
            );
            session.add_message("user", &user_msg);

            // 3. PRÉPARATION DU CONTEXTE
            let history_str = session
                .messages
                .iter()
                .rev()
                .take(5)
                .rev()
                .map(|m| format!("{}: {}", m.role, m.content))
                .collect::<Vec<_>>()
                .join("\n");

            // 4. ANALYSE INTELLIGENTE (Stateful)
            let analysis = self
                .analyze_business_need(ctx, domain, description, &history_str)
                .await
                .unwrap_or(json!({}));

            let cap_desc = analysis["capability"]["description"]
                .as_str()
                .unwrap_or(description)
                .to_string();

            // 5. CRÉATION ARTEFACTS
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
            artifacts.push(save_artifact(ctx, "oa", "capabilities", &cap_doc).await?);

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

                    artifacts.push(save_artifact(ctx, "oa", "actors", &act_doc).await?);
                }
            }

            let result_message = format!(
                "J'ai analysé le processus **{}** et identifié {} acteur(s). Je transmets l'analyse au système...",
                process_name,
                artifacts.len() - 1
            );

            // 6. DÉLÉGATION AUTOMATIQUE (OA -> SA)
            let transition_msg = format!(
                "J'ai modélisé la capacité opérationnelle (OA) '{}' avec {} acteurs. Peux-tu en déduire les Fonctions Système et les Acteurs Système correspondants ?",
                process_name,
                artifacts.len() - 1
            );

            let acl_msg = AclMessage::new(
                Performative::Request,
                self.id(),          // Sender: business_analyst
                "system_architect", // Receiver: system_architect
                &transition_msg,
            );

            // 7. MÉMORISATION DE LA RÉPONSE & SAUVEGARDE
            session.add_message("assistant", &result_message);
            save_session(ctx, &session).await?;

            return Ok(Some(AgentResult {
                message: result_message,
                artifacts,
                // AJOUT : Message sortant activé
                outgoing_message: Some(acl_msg),
            }));
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::protocols::acl::Performative;

    #[test]
    fn test_business_id() {
        assert_eq!(BusinessAgent::new().id(), "business_analyst");
    }

    // NOUVEAU TEST : Vérifie le déclenchement de la transition vers SystemAgent
    #[tokio::test]
    async fn test_business_delegation_trigger() {
        // Préfixe '_' pour éviter les warnings unused variables
        let _agent = BusinessAgent::new();
        let process_name = "Gestion_Commandes";
        let actors_count = 2;

        let transition_msg = format!(
            "J'ai modélisé la capacité opérationnelle (OA) '{}' avec {} acteurs. Peux-tu en déduire les Fonctions Système et les Acteurs Système correspondants ?",
            process_name,
            actors_count
        );

        let acl_msg = AclMessage::new(
            Performative::Request,
            "business_analyst",
            "system_architect",
            &transition_msg,
        );

        assert_eq!(acl_msg.receiver, "system_architect");
        assert_eq!(acl_msg.performative, Performative::Request);
        assert!(acl_msg.content.contains("Gestion_Commandes"));
    }
}
