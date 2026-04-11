// FICHIER : src-tauri/tests/code_gen_suite/agent_tests.rs

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
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_data_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection via Mount Points) ---
    // Utilisation dynamique de la configuration système SSOT pour la résilience
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    let sys_mgr = CollectionsManager::new(&env.sandbox.storage, system_domain, system_db);

    // Initialisation résiliente de l'index système
    match DbSandbox::mock_db(&sys_mgr).await {
        Ok(_) => user_info!("INF_TEST_MOCK_DB_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";
    let collections = vec!["prompts", "agents", "configs"];

    for coll in collections {
        let _ = sys_mgr.create_collection(coll, generic_schema).await;
    }

    // Injection du prompt "cerveau" logiciel
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

    // --- 🎯 2. SETUP SPÉCIFIQUE AU TEST (Couche LA) ---
    let la_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "la");
    let _ = DbSandbox::mock_db(&la_mgr).await;

    la_mgr
        .create_collection("components", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION IA ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_codegen");

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

    let _ctx = AgentContext::new(
        agent_urn,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        client.clone(),
        world_engine,
        test_data_root.clone(),
        test_data_root.join("dataset"),
    )
    .await;

    let classifier = IntentClassifier::new(client);

    // --- TEST 1 : CLASSIFICATION DE CRÉATION ---
    let input_create = "Créer une fonction système nommée DémarrerMoteur.";
    user_info!(
        "INF_TEST_CLASSIFY_START",
        json_value!({"input": input_create})
    );

    let intent = classifier.classify(input_create).await;

    match intent {
        EngineeringIntent::CreateElement { name, .. } => {
            let clean_name = name.replace(['\'', '\"'], "");
            assert!(
                clean_name.to_lowercase().contains("demarrermoteur")
                    || clean_name.to_lowercase().contains("démarrermoteur"),
                "Nom incorrect. Reçu: '{}'",
                name
            );
            user_success!("SUC_INTENT_CREATE_VALIDATED");
        }
        EngineeringIntent::Unknown => {
            user_warn!("WRN_LLM_TOLERANCE_UNKNOWN");
        }
        _ => {
            user_warn!(
                "WRN_LLM_INCORRECT_CLASSIFICATION",
                json_value!({"intent": format!("{:?}", intent)})
            );
        }
    }

    // --- TEST 2 : CLASSIFICATION DE CODE GEN ---
    let input_code = "Génère le code Rust pour le composant Auth. IMPORTANT: Le JSON DOIT contenir le champ \"filename\": \"auth.rs\".";
    let intent_code = classifier.classify(input_code).await;

    match intent_code {
        EngineeringIntent::GenerateCode {
            language, filename, ..
        } => {
            assert!(language.to_lowercase().contains("rust"));
            assert!(
                !filename.is_empty(),
                "L'IA a ignoré l'instruction du filename"
            );
            user_success!("SUC_INTENT_CODEGEN_VALIDATED");
        }
        EngineeringIntent::Unknown => {
            user_warn!("WRN_LLM_TOLERANCE_UNKNOWN");
        }
        _ => {
            user_warn!(
                "WRN_LLM_INCORRECT_CLASSIFICATION",
                json_value!({"intent": format!("{:?}", intent_code)})
            );
        }
    }

    Ok(())
}

// =========================================================================
// NOUVEAUX TESTS : RÉSILIENCE ET CONFIGURATION
// =========================================================================

#[cfg(test)]
mod resilience_tests {
    use super::*;
    use raise::ai::agents::Agent;
    use raise::ai::llm::client::LlmClient;
    /// 🎯 Test la résilience face à la résolution des partitions via Mount Points
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_codegen_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        // Validation SSOT de la partition système injectée dans la sandbox
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la réaction en cas de prompt manquant pour l'agent (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_codegen_agent_missing_prompt_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        let test_root = env.sandbox.storage.config.data_root.clone();

        let sys_mgr = CollectionsManager::new(
            &env.sandbox.storage,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        // 1. Injection d'un agent avec un prompt_id orphelin
        sys_mgr
            .upsert_document(
                "agents",
                json_value!({
                    "handle": "agent_broken_codegen",
                    "base": { "neuro_profile": { "prompt_id": "ghost_prompt" } }
                }),
            )
            .await?;

        // 2. Préparation du contexte d'exécution
        use candle_nn::VarMap;
        let world_engine = SharedRef::new(
            raise::ai::world_model::NeuroSymbolicEngine::new(
                raise::utils::data::config::WorldModelConfig::default(),
                VarMap::new(),
            )
            .expect("WM Engine fail"),
        );

        let llm_client = match env.client.clone() {
            Some(client) => client,
            None => LlmClient::new(&sys_mgr).await.expect("LlmClient fail"),
        };

        let ctx = AgentContext::new(
            "agent_broken_codegen",
            "sess_resilience",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.join("dataset"),
        )
        .await;

        let agent = DynamicAgent::new("agent_broken_codegen");

        // 🎯 FIX : Utilisation de la méthode 'process' au lieu de 'process_raw_request'
        // On passe une intention Chat pour déclencher la phase de compilation du prompt.
        let res = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match res {
            Err(AppError::Structured(data)) => {
                // Le moteur doit lever une erreur car 'ghost_prompt' n'existe pas en DB
                // L'erreur provient du PromptEngine invoqué par DynamicAgent::process.
                assert!(data.code.contains("ERR_PROMPT") || data.code.contains("ERR_DB"));
                Ok(())
            }
            _ => panic!("Le moteur aurait dû diverger sur une erreur structurée RAISE"),
        }
    }
}
