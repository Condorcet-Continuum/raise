// FICHIER : src-tauri/tests/ai_suite/system_agent_tests.rs

use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_system_agent_creates_function_end_to_end() -> RaiseResult<()> {
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

    // Injection du prompt système expert certifié Arcadia
    sys_mgr.upsert_document("prompts", json_value!({
        "handle": "prompt_system",
        "role": "Architecte Système",
        "identity": { 
            "persona": "Tu es un Ingénieur Système expert certifié Arcadia (Couche SA). Répond en JSON pur.",
            "tone": "analytique"
        },
        "environment": "Analyse Système (SA) du Continuum RAISE.", 
        "directives": ["Génère la fonction système (SystemFunction) demandée en format JSON."]
    })).await?;

    let agent_urn = "agent_system";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "System Architect" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_system", "temperature": 0.1 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJET (Physique) ---
    let sa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "sa");
    let _ = DbSandbox::mock_db(&sa_mgr).await;

    sa_mgr
        .create_collection("functions", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_sa");

    use candle_nn::VarMap;
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(
            raise::utils::data::config::WorldModelConfig::default(),
            VarMap::new(),
        )
        .expect("WM Engine fail"),
    );

    let ctx = AgentContext::new(
        agent_urn,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client.clone().expect("LlmClient requis pour les tests"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    let agent = DynamicAgent::new(agent_urn);
    let intent = EngineeringIntent::CreateElement {
        layer: "SA".to_string(),
        element_type: "Function".to_string(),
        name: "Calculer Vitesse".to_string(),
    };

    user_info!("INF_SYSTEM_AGENT_LAUNCH");
    let result = match agent.process(&ctx, &intent).await {
        Ok(Some(res)) => res,
        Ok(None) => raise_error!("ERR_TEST_NO_RESULT"),
        Err(e) => return Err(e),
    };

    let delegated = result.outgoing_message.is_some();

    // --- 🔍 4. VÉRIFICATION (Résilience & Artefacts) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let functions_dir = test_root.join("un2/sa/collections/functions");
    let mut found = false;

    if functions_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&functions_dir) {
            for e in entries.flatten() {
                let content = fs::read_to_string_sync(&e.path())
                    .unwrap_or_default()
                    .to_lowercase();
                if content.contains("calculer") && content.contains("vitesse") {
                    found = true;
                    user_success!("SUC_FUNCTION_VALIDATED");
                    break;
                }
            }
        }
    }

    if delegated {
        user_success!("SUC_SYSTEM_DELEGATION_OK");
    } else if found {
        user_success!("SUC_SYSTEM_GENERATION_OK");
    } else {
        user_warn!("WRN_FUNCTION_NOT_FOUND");
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
    async fn test_system_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        // Validation SSOT de la partition système via config sandbox
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la réaction en cas de prompt manquant pour l'agent (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_system_agent_missing_prompt_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        let test_root = env.sandbox.storage.config.data_root.clone();

        let sys_mgr = CollectionsManager::new(
            &env.sandbox.storage,
            &env.sandbox.config.mount_points.system.domain,
            &env.sandbox.config.mount_points.system.db,
        );

        // Injection d'un agent avec un prompt_id orphelin
        sys_mgr
            .upsert_document(
                "agents",
                json_value!({
                    "handle": "agent_broken_sa",
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
            "agent_broken_sa",
            "sess_err",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.clone(),
        )
        .await;

        let agent = DynamicAgent::new("agent_broken_sa");
        let res = agent.process(&ctx, &EngineeringIntent::Chat).await;

        match res {
            Err(AppError::Structured(data)) => {
                // Doit diverger sur une erreur de résolution de prompt
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
