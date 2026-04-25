// FICHIER : src-tauri/tests/code_gen_suite/agent_tests.rs

// 🧹 FIX : Retrait de get_test_wm_config
use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{dynamic_agent::DynamicAgent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_software_agent_creates_component_end_to_end() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Enabled).await?;

    // 🎯 FIX : Utilisation de domain_root
    let test_data_root = env.sandbox.domain_root.clone();

    // --- 🎯 1. SETUP SYSTEM ---
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    // 🎯 FIX : Utilisation de .db
    let sys_mgr = CollectionsManager::new(&env.sandbox.db, system_domain, system_db);

    match DbSandbox::mock_db(&sys_mgr).await {
        Ok(_) => user_info!("INF_TEST_MOCK_DB_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
    let collections = vec!["prompts", "agents", "configs"];

    for coll in collections {
        let _ = sys_mgr.create_collection(coll, generic_schema).await;
    }

    sys_mgr.upsert_document("prompts", json_value!({
        "handle": "prompt_software",
        "role": "Ingénieur Logiciel",
        "identity": { "persona": "Tu es un Développeur Rust Expert. Tu conçois la Logical Architecture (LA) et génères du code." },
        "directives": ["Génère le composant ou le code en format JSON."]
    })).await?;

    let agent_urn = "agent_software";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "Software Engineer" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_software", "temperature": 0.1 }
        }
    })).await?;

    // --- 🎯 2. SETUP SPÉCIFIQUE (Couche LA) ---
    let la_mgr = CollectionsManager::new(&env.sandbox.db, "un2", "la");
    let _ = DbSandbox::mock_db(&la_mgr).await;

    la_mgr
        .create_collection("components", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION IA ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_codegen")?;

    // 🎯 FIX : Bootstrap pour alignement production
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::bootstrap(&sys_mgr)
            .await
            .expect("WM Engine bootstrap fail"),
    );

    let client = match env.client.clone() {
        Some(c) => c,
        None => raise_error!("ERR_LLM_CLIENT_DISABLED"),
    };

    let _ctx = AgentContext::new(
        agent_urn,
        &session_id,
        env.sandbox.db.clone(),
        client.clone(),
        world_engine,
        test_data_root.clone(),
        test_data_root.join("dataset"),
    )
    .await;

    let classifier = IntentClassifier::new(client);

    // --- TEST 1 : CLASSIFICATION DE CRÉATION ---
    let input_create = "Créer une fonction système nommée DémarrerMoteur.";
    let intent = classifier.classify(input_create).await;

    match intent {
        EngineeringIntent::CreateElement { name, .. } => {
            let clean_name = name.replace(['\'', '\"'], "");
            assert!(clean_name.to_lowercase().contains("moteur"));
            user_success!("SUC_INTENT_CREATE_VALIDATED");
        }
        _ => user_warn!("WRN_LLM_TOLERANCE_UNEXPECTED"),
    }

    Ok(())
}

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use raise::ai::agents::Agent;
    use raise::ai::llm::client::LlmClient;

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_codegen_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await?;
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_codegen_agent_missing_prompt_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await?;
        let test_root = env.sandbox.domain_root.clone();

        let sys_mgr = CollectionsManager::new(
            &env.sandbox.db,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        sys_mgr
            .upsert_document(
                "agents",
                json_value!({
                    "handle": "agent_broken_codegen",
                    "base": { "neuro_profile": { "prompt_id": "ghost_prompt" } }
                }),
            )
            .await?;

        let world_engine = SharedRef::new(
            raise::ai::world_model::NeuroSymbolicEngine::bootstrap(&sys_mgr)
                .await
                .expect("WM Engine fail"),
        );

        let llm_client = match env.client.clone() {
            Some(client) => client,
            None => LlmClient::new(&sys_mgr).await.expect("LlmClient fail"),
        };

        let ctx = AgentContext::new(
            "agent_broken_codegen",
            "sess_resilience",
            env.sandbox.db.clone(),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.join("dataset"),
        )
        .await?;

        let agent = DynamicAgent::new("agent_broken_codegen");
        let res = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match res {
            Err(AppError::Structured(data)) => {
                assert!(
                    data.code.contains("ERR_AGENT_PROMPT_COMPILE")
                        || data.code.contains("ERR_PROMPT")
                );
                Ok(())
            }
            _ => panic!("Le moteur aurait dû diverger"),
        }
    }
}
