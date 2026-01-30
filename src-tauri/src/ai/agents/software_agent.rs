// FICHIER : src-tauri/src/ai/agents/software_agent.rs

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use uuid::Uuid;

use super::intent_classifier::EngineeringIntent;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult, CreatedArtifact};

// Import du protocole ACL
use crate::ai::protocols::acl::{AclMessage, Performative};

// AJOUT : Import du protocole MCP et de l'outil FileSystem
use crate::ai::protocols::mcp::{McpTool, McpToolCall};
use crate::ai::tools::FileWriteTool;

use crate::ai::llm::client::LlmBackend;
use crate::ai::nlp::entity_extractor;

#[derive(Default)]
pub struct SoftwareAgent;

impl SoftwareAgent {
    pub fn new() -> Self {
        Self {}
    }

    async fn ask_llm(&self, ctx: &AgentContext, system: &str, user: &str) -> Result<String> {
        ctx.llm
            .ask(LlmBackend::LocalLlama, system, user)
            .await
            .map_err(|e| anyhow!("Erreur LLM : {}", e))
    }

    async fn enrich_logical_component(
        &self,
        ctx: &AgentContext,
        name: &str,
        description: &str,
        history_context: &str,
    ) -> Result<serde_json::Value> {
        let entities = entity_extractor::extract_entities(name);
        let mut nlp_hint = String::new();
        if !entities.is_empty() {
            nlp_hint.push_str("\n[VOCABULAIRE]: ");
            for entity in entities {
                nlp_hint.push_str(&format!("{}, ", entity.text));
            }
        }

        let system_prompt = "Tu es un Architecte Logiciel. Génère JSON valide.";
        let user_prompt = format!(
            "=== HISTORIQUE ===\n{}\n\n=== TÂCHE ===\nCrée Composant LA.\nNom: {}\nDesc: {}\n{}\nJSON: {{ \"name\": \"str\", \"implementation_language\": \"rust|cpp\" }}",
            history_context, name, description, nlp_hint
        );

        let response = self.ask_llm(ctx, system_prompt, &user_prompt).await?;
        let clean_json = extract_json_from_llm(&response);

        let mut data: serde_json::Value = serde_json::from_str(&clean_json)
            .unwrap_or(json!({ "name": name, "description": description }));

        data["id"] = json!(Uuid::new_v4().to_string());
        data["layer"] = json!("LA");
        data["type"] = json!("LogicalComponent");
        data["createdAt"] = json!(chrono::Utc::now().to_rfc3339());

        Ok(data)
    }
}

#[async_trait]
impl Agent for SoftwareAgent {
    fn id(&self) -> &'static str {
        "software_engineer"
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
                layer: _,
                element_type,
                name,
            } => {
                // A. Mutation (Ajout Message User)
                session.add_message(
                    "user",
                    &format!("Create Logical Component: {} ({})", name, element_type),
                );

                // B. Lecture (Calcul Historique)
                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                // C. Appel LLM
                let doc = self
                    .enrich_logical_component(
                        ctx,
                        name,
                        &format!("Type: {}", element_type),
                        &history_str,
                    )
                    .await?;

                let artifact = save_artifact(ctx, "la", "components", &doc)?;

                // D. DÉLÉGATION -> EPBS (Configuration Manager)
                let transition_msg = format!(
                    "J'ai créé le composant logiciel '{}'. Merci de créer l'Article de Configuration (CI) associé et de lui attribuer un Part Number.",
                    name
                );
                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),               // Sender
                    "configuration_manager", // Receiver (EpbsAgent)
                    &transition_msg,
                );

                let msg = format!(
                    "Composant logiciel **{}** modélisé. Demande de création CI envoyée.",
                    name
                );

                // E. Mutation (Ajout Réponse Assistant)
                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![artifact],
                    outgoing_message: Some(acl_msg),
                }))
            }
            EngineeringIntent::GenerateCode {
                language,
                context,
                filename,
            } => {
                // A. Mutation
                session.add_message(
                    "user",
                    &format!("Generate code for {} in {}", context, language),
                );

                // B. Lecture
                let history_str = session
                    .messages
                    .iter()
                    .rev()
                    .take(5)
                    .rev()
                    .map(|m| format!("{}: {}", m.role, m.content))
                    .collect::<Vec<_>>()
                    .join("\n");

                let user_prompt = format!(
                    "=== HISTORIQUE (Pour contexte) ===\n{}\n\n=== CODE ===\nGénère le code pour: {}\nLangage: {}",
                    history_str, context, language
                );

                let code = self
                    .ask_llm(
                        ctx,
                        "Expert Code. Pas de markdown. Utilise le contexte.",
                        &user_prompt,
                    )
                    .await?;

                let clean_code = code.replace("```rust", "").replace("```", "");

                // --- INTEGRATION MCP : REMPLACEMENT DE L'ÉCRITURE DIRECTE ---

                // 1. Instanciation de l'outil avec la racine du domaine (Sandbox)
                let fs_tool = FileWriteTool::new(ctx.paths.domain_root.clone());

                // 2. Préparation de l'appel (Tool Call)
                let relative_path = format!("src-gen/{}", filename);
                let call = McpToolCall::new(
                    "fs_write",
                    json!({
                        "path": relative_path,
                        "content": clean_code.trim()
                    }),
                );

                // 3. Exécution sécurisée
                let result = fs_tool.execute(call).await;

                if result.is_error {
                    return Err(anyhow!("Échec écriture MCP : {:?}", result.content));
                }

                // -----------------------------------------------------------

                let full_path = ctx.paths.domain_root.join(&relative_path);

                // C. DÉLÉGATION -> TRANSVERSE (Quality Manager)
                let transition_msg = format!(
                    "Le code source '{}' a été généré via l'outil MCP fs_write. Peux-tu préparer le plan de test unitaire associé ?",
                    filename
                );
                let acl_msg = AclMessage::new(
                    Performative::Request,
                    self.id(),
                    "quality_manager", // Receiver (TransverseAgent)
                    &transition_msg,
                );

                let msg = format!(
                    "Code source généré dans **{}** via protocole standardisé.",
                    filename
                );

                // D. Mutation
                session.add_message("assistant", &msg);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![CreatedArtifact {
                        id: filename.clone(),
                        name: filename.clone(),
                        layer: "CODE".to_string(),
                        element_type: "SourceFile".to_string(),
                        path: full_path.to_string_lossy().to_string(),
                    }],
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
    fn test_software_agent_id() {
        assert_eq!(SoftwareAgent::new().id(), "software_engineer");
    }

    #[tokio::test]
    async fn test_software_delegation_triggers() {
        let _agent = SoftwareAgent::new();

        // 1. Test Composant -> EPBS
        let msg_comp = AclMessage::new(
            Performative::Request,
            "software_engineer",
            "configuration_manager",
            "Create CI",
        );
        assert_eq!(msg_comp.receiver, "configuration_manager");

        // 2. Test Code -> Quality
        let msg_code = AclMessage::new(
            Performative::Request,
            "software_engineer",
            "quality_manager",
            "Create Tests",
        );
        assert_eq!(msg_code.receiver, "quality_manager");
    }
}
