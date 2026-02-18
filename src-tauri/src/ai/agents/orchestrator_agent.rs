// FICHIER : src-tauri/src/ai/agents/orchestrator_agent.rs

use crate::utils::{async_trait, prelude::*};

use super::intent_classifier::EngineeringIntent;
use super::tools::{load_session, save_session};
use super::{Agent, AgentContext, AgentResult};
use crate::ai::llm::client::LlmBackend;

#[derive(Default)]
pub struct OrchestratorAgent;

impl OrchestratorAgent {
    pub fn new() -> Self {
        Self
    }

    /// Demande de clarification à l'utilisateur via le LLM
    async fn handle_clarification(&self, ctx: &AgentContext, user_input: &str) -> Result<String> {
        let sys = "Tu es l'Orchestrateur de RAISE. Ton rôle est de coordonner les agents MBSE (Business, System, Software, Hardware, Transverse).";
        let user = format!("L'utilisateur a dit : '{}'. C'est trop vague pour une action d'ingénierie. Demande poliment quelle étape (spécification, conception, code, test) il souhaite aborder.", user_input);

        ctx.llm
            .ask(LlmBackend::LocalLlama, sys, &user)
            .await
            .map_err(|e| AppError::Validation(e.to_string()))
    }
}

#[async_trait]
impl Agent for OrchestratorAgent {
    fn id(&self) -> &'static str {
        "orchestrator_agent"
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> Result<Option<AgentResult>> {
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            // Cas 1 : L'intention est un Chat ou Inconnue -> Clarification
            EngineeringIntent::Unknown | EngineeringIntent::Chat => {
                let last_msg = session
                    .messages
                    .last()
                    .map(|m| m.content.as_str())
                    .unwrap_or("");
                let reply = self.handle_clarification(ctx, last_msg).await?;

                session.add_message("assistant", &reply);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: reply,
                    artifacts: vec![],
                    outgoing_message: None,
                }))
            }

            // Cas 2 : L'intention est déjà spécifiée
            _ => {
                let msg = format!("Je coordonne l'exécution de l'intention : {:?}", intent);
                Ok(Some(AgentResult {
                    message: msg,
                    artifacts: vec![],
                    outgoing_message: None,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::llm::client::LlmClient;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::{io::tempdir, Arc};

    #[test]
    fn test_orchestrator_id() {
        assert_eq!(OrchestratorAgent::new().id(), "orchestrator_agent");
    }

    #[tokio::test]
    async fn test_orchestrator_clarification_logic() {
        let dir = tempdir().unwrap();
        let domain_root = dir.path().to_path_buf();
        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        let ctx = AgentContext::new(
            "tester",
            "sess_orch_01",
            db,
            llm,
            domain_root.clone(),
            domain_root.clone(),
        );

        let agent = OrchestratorAgent::new();
        let intent = EngineeringIntent::Unknown;

        let result = agent.process(&ctx, &intent).await;

        match result {
            Ok(Some(res)) => {
                assert!(!res.message.is_empty());
                let session = crate::ai::agents::tools::load_session(&ctx).await.unwrap();
                assert!(session.messages.iter().any(|m| m.role == "assistant"));
            }
            Err(e) => println!("Note: Erreur LLM attendue si Ollama off: {}", e),
            _ => panic!("Echec du test de clarification"),
        }
    }

    #[tokio::test]
    async fn test_orchestrator_routing_feedback() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let db = Arc::new(StorageEngine::new(config));
        let llm = LlmClient::new("http://localhost:11434", "dummy", None);

        let ctx = AgentContext::new("t", "s", db, llm, dir.path().into(), dir.path().into());
        let agent = OrchestratorAgent::new();

        let intent = EngineeringIntent::CreateElement {
            layer: "SA".into(),
            element_type: "System".into(),
            name: "Radar".into(),
        };

        let result = agent.process(&ctx, &intent).await.unwrap().unwrap();
        assert!(result.message.contains("coordonne"));
    }
}
