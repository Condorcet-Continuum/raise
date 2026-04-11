// FICHIER : src-tauri/src/ai/agents/dynamic_agent.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

use super::intent_classifier::EngineeringIntent;
use super::prompt_engine::PromptEngine;
use super::tools::{extract_json_from_llm, load_session, save_artifact, save_session};
use super::{Agent, AgentContext, AgentResult};

use crate::ai::llm::client::LlmBackend;

/// L'Agent Dynamique piloté par les données (Data-Driven).
pub struct DynamicAgent {
    handle: String,
}

impl DynamicAgent {
    pub fn new(handle: &str) -> Self {
        Self {
            handle: handle.to_string(),
        }
    }
}

#[async_interface]
impl Agent for DynamicAgent {
    fn id(&self) -> &str {
        &self.handle
    }

    async fn process(
        &self,
        ctx: &AgentContext,
        intent: &EngineeringIntent,
    ) -> RaiseResult<Option<AgentResult>> {
        let config = AppConfig::get();

        // 🎯 Rigueur : Utilisation des points de montage système
        let sys_manager = CollectionsManager::new(
            &ctx.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // 1. Charger la configuration de l'Agent via Match strict
        let agent_doc = match sys_manager.get_document("agents", &self.handle).await? {
            Some(doc) => doc,
            None => {
                raise_error!(
                    "ERR_AGENT_CONFIG_NOT_FOUND",
                    error = format!("Agent '{}' introuvable.", self.handle),
                    context =
                        json_value!({ "handle": self.handle, "mount": config.mount_points.system })
                );
            }
        };

        // 2. Extraire le prompt_id via Match strict
        let prompt_id = match agent_doc["base"]["neuro_profile"]["prompt_id"].as_str() {
            Some(id) => id,
            None => {
                raise_error!(
                    "ERR_AGENT_MISSING_PROMPT",
                    error = "prompt_id absent du neuro_profile.",
                    context = json_value!({ "agent": self.handle })
                );
            }
        };

        // 3. Compiler le System Prompt via le registre système
        let prompt_engine = PromptEngine::new(
            ctx.db.clone(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let system_prompt = prompt_engine.compile(prompt_id, None).await?;

        // 4. Charger la session
        let mut session = match load_session(ctx).await {
            Ok(s) => s,
            Err(_) => super::AgentSession::new(&ctx.session_id, self.id()),
        };

        let intent_text = format!("{:?}", intent);
        session.add_message("user", &intent_text);

        let history_str = session
            .messages
            .iter()
            .rev()
            .take(10)
            .rev()
            .map(|m| format!("{}: {}", m.role, m.content))
            .collect::<Vec<_>>()
            .join("\n");

        let user_prompt = format!(
            "=== HISTORIQUE ===\n{}\n\n=== TÂCHE ===\n{}",
            history_str, intent_text
        );

        // 5. Exécution neuronale
        let agent_name = agent_doc["base"]["name"]["fr"]
            .as_str()
            .unwrap_or(&self.handle);
        user_info!(
            "SYS_INFO",
            json_value!({ "message": format!("🧠 Agent : {}", agent_name) })
        );

        let response = ctx
            .llm
            .ask(LlmBackend::LocalLlama, &system_prompt, &user_prompt)
            .await?;

        // 6. Extraction et Persistance des Artefacts
        let clean_json = extract_json_from_llm(&response);
        session.add_message("assistant", &clean_json);
        save_session(ctx, &session).await?;

        let mut artifacts = vec![];
        let parsed: JsonValue = json::deserialize_from_str(&clean_json).unwrap_or(json_value!({}));
        let mut docs_to_save = vec![];

        match parsed {
            JsonValue::Array(arr) => docs_to_save.extend(arr),
            JsonValue::Object(obj) if !obj.is_empty() => docs_to_save.push(JsonValue::Object(obj)),
            _ => {}
        }

        for mut doc in docs_to_save {
            let layer = doc["layer"].as_str().unwrap_or("").to_string();
            let element_type = doc["type"].as_str().unwrap_or("").to_string();

            if layer.is_empty() || element_type.is_empty() {
                continue;
            }

            // Garantie d'intégrité de l'identifiant v2
            if let Some(obj) = doc.as_object_mut() {
                if !obj.contains_key("_id") {
                    obj.insert(
                        "_id".to_string(),
                        json_value!(UniqueId::new_v4().to_string()),
                    );
                }
            }

            if let Ok(artifact) = save_artifact(ctx, &doc).await {
                artifacts.push(artifact);
            }
        }

        Ok(Some(AgentResult {
            message: format!("Cycle terminé. {} artefacts persistés.", artifacts.len()),
            artifacts,
            outgoing_message: None,
        }))
    }
}

// =========================================================================
// TESTS UNITAIRES (Restauration intégrale et Gardes CUDA)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::llm::client::LlmClient;
    use crate::ai::world_model::NeuroSymbolicEngine;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};
    use candle_nn::VarMap;

    async fn setup_test_ctx(sandbox: &AgentDbSandbox) -> AgentContext {
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        inject_mock_component(&manager, "llm", json_value!({})).await;
        let llm = match LlmClient::new(&manager).await {
            Ok(c) => c,
            Err(e) => panic!("Erreur LLM : {:?}", e),
        };
        let world_engine = SharedRef::new(
            NeuroSymbolicEngine::new(WorldModelConfig::default(), VarMap::new()).unwrap(),
        );

        AgentContext::new(
            "test_agent",
            "sess_123",
            sandbox.db.clone(),
            llm,
            world_engine,
            sandbox.domain_root.clone(),
            sandbox.domain_root.clone(),
        )
        .await
    }

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_id_mapping() {
        let agent = DynamicAgent::new("agent_modeling");
        assert_eq!(agent.id(), "agent_modeling");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_err_agent_not_found() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let ctx = setup_test_ctx(&sandbox).await;
        let agent = DynamicAgent::new("agent_fantome");

        let result = agent.process(&ctx, &EngineeringIntent::Chat).await;
        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_AGENT_CONFIG_NOT_FOUND");
                Ok(())
            }
            _ => panic!("Attendu: ERR_AGENT_CONFIG_NOT_FOUND"),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_err_missing_prompt_id() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let ctx = setup_test_ctx(&sandbox).await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        manager
            .create_collection(
                "agents",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;
        manager
            .insert_raw(
                "agents",
                &json_value!({
                    "_id": "invalid_agent",
                    "base": { "name": {"fr": "Sans Prompt"}, "neuro_profile": {} }
                }),
            )
            .await?;

        let agent = DynamicAgent::new("invalid_agent");
        let result = agent.process(&ctx, &EngineeringIntent::Chat).await;
        match result {
            Err(AppError::Structured(data)) => {
                assert_eq!(data.code, "ERR_AGENT_MISSING_PROMPT");
                Ok(())
            }
            _ => panic!("Attendu: ERR_AGENT_MISSING_PROMPT"),
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_successful_execution_and_session_init() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let ctx = setup_test_ctx(&sandbox).await;

        let sys_manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let ws_manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.modeling.domain,
            &config.mount_points.modeling.db,
        );

        AgentDbSandbox::mock_db(&ws_manager).await?;

        let generic_uri = "db://_system/_system/schemas/v1/db/generic.schema.json";

        // 1. Initialisation des collections
        for col in &["prompts", "agents"] {
            sys_manager.create_collection(col, generic_uri).await?;
        }

        // 🎯 FIX : C'est "session_agents" qui est codé en dur dans save_session !
        sys_manager
            .create_collection("session_agents", generic_uri)
            .await?;
        ws_manager
            .create_collection("session_agents", generic_uri)
            .await?;

        // 2. Injection des données (avec le prompt Parfait !)
        sys_manager
            .insert_raw(
                "prompts",
                &json_value!({
                    "_id": "p_test",
                    "role": "system",
                    "environment": "Environnement de test unitaire",
                    "identity": {
                        "persona": "Test"
                    },
                    "directives": ["OK"]
                }),
            )
            .await?;

        sys_manager
            .insert_raw(
                "agents",
                &json_value!({
                    "_id": "agent_ok",
                    "base": {
                        "name": {"fr": "Agent OK"},
                        "neuro_profile": { "prompt_id": "p_test" }
                    }
                }),
            )
            .await?;

        // 3. Exécution de l'agent
        let agent = DynamicAgent::new("agent_ok");
        let result = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match result {
            Ok(_) => {}
            Err(e) => panic!("❌ L'exécution de l'agent a échoué : {:?}", e),
        };

        // 4. Vérification dans "session_agents"
        let query = crate::json_db::query::Query::new("session_agents"); // 🎯 FIX

        let res_sys = crate::json_db::query::QueryEngine::new(&sys_manager)
            .execute_query(query.clone())
            .await?;
        let res_ws = crate::json_db::query::QueryEngine::new(&ws_manager)
            .execute_query(query)
            .await?;

        assert!(
            !res_sys.documents.is_empty() || !res_ws.documents.is_empty(),
            "La collection 'session_agents' est vide partout. save_session n'a pas écrit."
        );

        Ok(())
    }
}
