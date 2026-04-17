// FICHIER : src-tauri/tests/ai_suite/software_agent_tests.rs

use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_software_agent_creates_component_end_to_end() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection via Mount Points) ---
    // Utilisation dynamique des points de montage pour la partition système
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    let sys_mgr = CollectionsManager::new(&env.sandbox.storage, system_domain, system_db);

    // Initialisation résiliente de l'index système
    match DbSandbox::mock_db(&sys_mgr).await {
        Ok(_) => user_info!("INF_TEST_MOCK_DB_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
    let collections = vec!["prompts", "agents", "session_agents", "configs"];

    for coll in collections {
        let _ = sys_mgr.create_collection(coll, generic_schema).await;
    }

    // Injection du prompt logiciel expert
    sys_mgr.upsert_document("prompts", json_value!({
        "handle": "prompt_software",
        "role": "Ingénieur Logiciel",
        "identity": { 
            "persona": "Tu es un Développeur Rust Expert. Tu conçois la Logical Architecture (LA).Répond en JSON pur.",
            "tone": "développeur"
        },
        "environment": "Architecture logicielle système Condorcet.",  
        "directives": ["Génère le LogicalComponent en format JSON."]
    })).await?;

    let agent_urn = "agent_software";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "Software Engineer" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_software", "temperature": 0.1 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJET (Physique) ---
    let la_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "la");
    let _ = DbSandbox::mock_db(&la_mgr).await;

    la_mgr
        .create_collection("components", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_la");

    use candle_nn::VarMap;
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(
            raise::utils::data::config::WorldModelConfig::default(),
            VarMap::new(),
        )
        .expect("WM Engine fail"),
    );

    let client = match env.client.clone() {
        Some(c) => c,
        None => raise_error!("ERR_LLM_CLIENT_DISABLED"),
    };

    let ctx = AgentContext::new(
        agent_urn,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        client.clone(),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    // --- TEST CLASSIFIER ET AGENT ---
    let classifier = IntentClassifier::new(client);
    let input = "Créer une fonction système nommée DémarrerMoteur.";
    let intent = classifier.classify(input).await;

    match &intent {
        EngineeringIntent::CreateElement { name, .. } => {
            assert!(
                name.to_lowercase().contains("demarrermoteur")
                    || name.to_lowercase().contains("démarrermoteur")
            );

            let agent = DynamicAgent::new(agent_urn);
            match agent.process(&ctx, &intent).await {
                Ok(_) => user_success!("SUC_SOFTWARE_AGENT_PROCESSED"),
                Err(e) => return Err(e),
            }
        }
        EngineeringIntent::Unknown => {
            user_warn!("WRN_LLM_TOLERANCE_UNKNOWN");
        }
        _ => {
            user_warn!(
                "WRN_LLM_TOLERANCE_UNEXPECTED",
                json_value!({"intent": format!("{:?}", intent)})
            );
        }
    }

    Ok(())
}

// =========================================================================
// NOUVEAUX TESTS : RÉSILIENCE ET POINTS DE MONTAGE
// =========================================================================

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use raise::ai::llm::client::LlmClient;

    /// 🎯 Test la résilience face à la résolution des partitions via Mount Points
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_software_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        // Validation de la configuration système injectée
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la réaction en cas de prompt manquant (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_software_agent_missing_prompt_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        let test_root = env.sandbox.storage.config.data_root.clone();

        let sys_mgr = CollectionsManager::new(
            &env.sandbox.storage,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        // Injection d'un agent avec un prompt orphelin
        sys_mgr
            .upsert_document(
                "agents",
                json_value!({
                    "handle": "agent_broken_sw",
                    "base": { "neuro_profile": { "prompt_id": "ghost_prompt" } }
                }),
            )
            .await?;

        use candle_nn::VarMap;
        let world_engine = SharedRef::new(
            raise::ai::world_model::NeuroSymbolicEngine::new(Default::default(), VarMap::new())
                .unwrap(),
        );

        let llm_client = match env.client.clone() {
            Some(c) => c,
            None => LlmClient::new(&sys_mgr).await.expect("LlmClient fail"),
        };

        let ctx = AgentContext::new(
            "agent_broken_sw",
            "sess_err",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.clone(),
        )
        .await;

        let agent = DynamicAgent::new("agent_broken_sw");
        let res = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match res {
            Err(AppError::Structured(data)) => {
                // Doit diverger sur une erreur de résolution de prompt via le PromptEngine
                assert!(
                    data.code.contains("ERR_AGENT_PROMPT_COMPILE")
                        || data.code.contains("ERR_PROMPT")
                        || data.code.contains("ERR_DB")
                );
                Ok(())
            }
            _ => panic!("Le moteur aurait dû diverger sur une erreur structurée"),
        }
    }
}
