// FICHIER : src-tauri/src/ai/agents/data_agent.rs

use crate::utils::{async_trait, data, prelude::*, Uuid};

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};

// AJOUT : Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct DataAgent;

impl DataAgent {
    pub fn new() -> Self {
        Self {}
    }

    /// Détermine quel agent doit être notifié de la création de cette donnée
    fn determine_target_agent(&self, name: &str) -> (&'static str, String) {
        let lower = name.to_lowercase();

        if lower.contains("client") || lower.contains("business") || lower.contains("metier") {
            ("business_analyst", format!("J'ai défini la donnée métier '{}'. Peux-tu vérifier si elle est utilisée dans les processus OA ?", name))
        } else if lower.contains("signal")
            || lower.contains("voltage")
            || lower.contains("hardware")
        {
            ("hardware_architect", format!("J'ai défini le signal physique '{}'. Merci de vérifier la compatibilité avec les interfaces matérielles.", name))
        } else if lower.contains("config") || lower.contains("param") || lower.contains("settings")
        {
            ("software_engineer", format!("J'ai défini la structure de configuration '{}'. Peux-tu l'implémenter dans le code ?", name))
        } else {
            ("system_architect", format!("J'ai défini la donnée système '{}'. Elle est disponible pour les échanges fonctionnels.", name))
        }
    }

    async fn call_llm(
        &self,
        ctx: &AgentContext,
        sys: &str,
        user: &str,
        doc_type: &str,
        original_name: &str,
    ) -> RaiseResult<Value> {
        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, sys, user)
            .await
            .map_err(|e| AppError::Validation(format!("LLM Err: {}", e)))?;

        let clean = extract_json_from_llm(&response);
        let mut doc: Value = data::parse(&clean).unwrap_or(json!({}));

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

    async fn enrich_class(
        &self,
        ctx: &AgentContext,
        name: &str,
        history_context: &str,
    ) -> RaiseResult<Value> {
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
            "=== HISTORIQUE ===\n{}\n\n=== TÂCHE ===\nNom: {}\n{}\nJSON: {{ \"name\": \"{}\", \"attributes\": [] }}",
            history_context, name, nlp_hint, name
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
            } if layer == "DATA" => {
                session.add_message(
                    "user",
                    &format!("Create Data Element: {} ({})", name, element_type),
                );

                // Calcul Historique
                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let (doc, collection) = match element_type.to_lowercase().as_str() {
                    "class" | "classe" => {
                        (self.enrich_class(ctx, name, &history_str).await?, "classes")
                    }
                    _ => (self.enrich_class(ctx, name, &history_str).await?, "classes"),
                };

                let artifact = save_artifact(ctx, "data", collection, &doc).await?;

                // 2. ROUTAGE DYNAMIQUE (Data -> All Layers)
                let (target_agent, msg_content) = self.determine_target_agent(name);

                let acl_msg = AclMessage::new(
                    Performative::Inform, // "Inform" car c'est une mise à disposition de donnée
                    self.id(),
                    target_agent,
                    &msg_content,
                );

                let msg = format!(
                    "Donnée **{}** définie. Notification envoyée à l'agent **{}**.",
                    name, target_agent
                );

                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    // AJOUT : Message sortant dynamique
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

    #[test]
    fn test_data_agent_sanity() {
        assert_eq!(DataAgent::new().id(), "data_architect");
    }

    // NOUVEAU TEST : Vérifie le routage dynamique vers les autres couches
    #[test]
    fn test_data_dynamic_routing() {
        let agent = DataAgent::new();

        // Cas 1 : Métier -> Business
        let (target1, _) = agent.determine_target_agent("Client_Business_Object");
        assert_eq!(target1, "business_analyst");

        // Cas 2 : Hardware -> Hardware
        let (target2, _) = agent.determine_target_agent("Analog_Signal_Voltage");
        assert_eq!(target2, "hardware_architect");

        // Cas 3 : Config -> Software
        let (target3, _) = agent.determine_target_agent("App_Config_Settings");
        assert_eq!(target3, "software_engineer");

        // Cas 4 : Défaut -> System
        let (target4, _) = agent.determine_target_agent("Generic_Data");
        assert_eq!(target4, "system_architect");
    }
}
