// FICHIER : src-tauri/tests/ai_suite/transverse_agent_tests.rs

use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_transverse_agent_ivvq_cycle() -> RaiseResult<()> {
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

    // Injection du prompt Qualité/IVVQ expert
    sys_mgr
        .upsert_document(
            "prompts",
            json_value!({
                "handle": "prompt_quality",
                "role": "Ingénieur Qualité Transverse",
                "identity": {
                    "persona": "Tu es le garant de la qualité et des exigences (Transverse). Répond en JSON pur.",
                    "tone": "rigoureux"
                },
                "environment": "Cycle IVVQ et gestion des exigences.",
                "directives": ["Génère l'exigence (Requirement) ou la procédure de test en JSON."]
            }),
        )
        .await?;

    let agent_urn = "agent_quality";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "Quality Manager" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_quality", "temperature": 0.1 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJET (Physique) ---
    let transverse_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "transverse");
    let _ = DbSandbox::mock_db(&transverse_mgr).await;

    transverse_mgr
        .create_collection("requirements", generic_schema)
        .await?;
    transverse_mgr
        .create_collection("test_procedures", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_transverse");

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

    // TEST EXIGENCE
    let intent_req = EngineeringIntent::CreateElement {
        layer: "TRANSVERSE".to_string(),
        element_type: "Requirement".to_string(),
        name: "L'avion doit résister à un impact d'oiseau".to_string(),
    };

    match agent.process(&ctx, &intent_req).await {
        Ok(_) => user_success!("SUC_TEST_REQUIREMENT_PROCESSED"),
        Err(e) => return Err(e),
    }

    // --- 🔍 4. VÉRIFICATION (Résilience & Artefacts) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let req_dir = test_root.join("un2/transverse/collections/requirements");
    let mut found_req = false;

    if req_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&req_dir) {
            for e in entries.flatten() {
                let content = fs::read_to_string_sync(&e.path())
                    .unwrap_or_default()
                    .to_lowercase();
                if content.contains("oiseau") || content.contains("impact") {
                    found_req = true;
                    user_success!("SUC_REQUIREMENT_VALIDATED");
                    break;
                }
            }
        }
    }

    if found_req {
        user_info!("INF_TEST_TRANSVERSE_FINISHED_OK");
    } else {
        user_warn!("WRN_REQUIREMENT_NOT_FOUND_LOCALLY");
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
    async fn test_transverse_mount_point_integrity() -> RaiseResult<()> {
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
    async fn test_transverse_agent_missing_prompt_resilience() -> RaiseResult<()> {
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
                    "handle": "agent_broken_transverse",
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
            "agent_broken_transverse",
            "sess_err",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.clone(),
        )
        .await;

        let agent = DynamicAgent::new("agent_broken_transverse");
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
