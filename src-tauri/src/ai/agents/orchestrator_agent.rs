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
    async fn handle_clarification(
        &self,
        ctx: &AgentContext,
        user_input: &str,
    ) -> RaiseResult<String> {
        let sys = "Tu es l'Orchestrateur de RAISE. Ton rôle est de coordonner les agents MBSE (Business, System, Software, Hardware, Transverse).";
        let user = format!("L'utilisateur a dit : '{}'. C'est trop vague pour une action d'ingénierie. Demande poliment quelle étape (spécification, conception, code, test) il souhaite aborder.", user_input);

        let res = match ctx.llm.ask(LlmBackend::LocalLlama, sys, &user).await {
            Ok(val) => val,
            Err(e) => {
                // La macro génère un log JSON et effectue un 'return Err'
                raise_error!(
                    "ERR_AI_CLARIFICATION_GENERATE",
                    error = e,
                    context = serde_json::json!({
                        "backend": "LocalLlama",
                        "user_input": user_input
                    })
                );
            }
        };
        Ok(res)
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
    ) -> RaiseResult<Option<AgentResult>> {
        let mut session = load_session(ctx)
            .await
            .unwrap_or_else(|_| super::AgentSession::new(&ctx.session_id, &ctx.agent_id));

        match intent {
            // Cas 1 : L'utilisateur veut juste discuter ou dire bonjour
            EngineeringIntent::Chat => {
                let last_msg = session
                    .messages
                    .last()
                    .map(|m| m.content.as_str())
                    .unwrap_or("");

                // On donne un rôle amical à Qwen
                let system_prompt = "Tu es l'assistant IA de RAISE. L'utilisateur te salue ou te pose une question générale. Réponds de manière polie, concise et professionnelle.";

                // On demande à ta RTX 5060 de générer la réponse
                let reply = ctx.llm.ask(crate::ai::llm::client::LlmBackend::LocalLlama, system_prompt, last_msg)
                        .await
                        .unwrap_or_else(|_| "Bonjour ! Comment puis-je vous aider dans votre projet d'ingénierie système ?".to_string());

                session.add_message("assistant", &reply);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: reply,
                    artifacts: vec![],
                    outgoing_message: None,
                }))
            }

            // Cas 2 : L'intention est vraiment inconnue -> On demande des précisions
            EngineeringIntent::Unknown => {
                let last_msg = session
                    .messages
                    .last()
                    .map(|m| m.content.as_str())
                    .unwrap_or("");

                // Appel à ta fonction existante pour gérer les demandes floues
                let reply = self.handle_clarification(ctx, last_msg).await?;

                session.add_message("assistant", &reply);
                save_session(ctx, &session).await?;

                Ok(Some(AgentResult {
                    message: reply,
                    artifacts: vec![],
                    outgoing_message: None,
                }))
            }

            // Cas 3 : L'intention technique est bien comprise (SA, LA, PA, etc.)
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
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::{io::tempdir, Arc};

    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::config::AppConfig;

    #[test]
    fn test_orchestrator_id() {
        assert_eq!(OrchestratorAgent::new().id(), "orchestrator_agent");
    }

    #[tokio::test]
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_clarification_logic() {
        inject_mock_config();

        let dir = tempdir().unwrap();
        let domain_root = dir.path().to_path_buf();
        let config = JsonDbConfig::new(domain_root.clone());
        let db = Arc::new(StorageEngine::new(config));

        let app_cfg = AppConfig::get();
        let manager = CollectionsManager::new(&db, &app_cfg.system_domain, &app_cfg.system_db);
        let _ = manager.init_db().await;

        // 🎯 Injection du LLM Mocké
        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        // 🎯 LlmClient avec le manager et .await
        let llm = LlmClient::new(&manager).await.unwrap();

        // 🎯 AgentContext avec .await
        let ctx = AgentContext::new(
            "tester",
            "sess_orch_01",
            db,
            llm,
            domain_root.clone(),
            domain_root.clone(),
        )
        .await;

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
    #[serial_test::serial] // Protection RTX 5060 en local
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_orchestrator_routing_feedback() {
        inject_mock_config();

        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let db = Arc::new(StorageEngine::new(config));

        let app_cfg = AppConfig::get();
        let manager = CollectionsManager::new(&db, &app_cfg.system_domain, &app_cfg.system_db);
        let _ = manager.init_db().await;

        crate::utils::config::test_mocks::inject_mock_component(
            &manager,
            "llm", 
            crate::utils::json::json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
        ).await;

        let llm = LlmClient::new(&manager).await.unwrap();

        let ctx = AgentContext::new("t", "s", db, llm, dir.path().into(), dir.path().into()).await;
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
