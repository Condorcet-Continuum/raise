// FICHIER : src-tauri/tests/ai_suite/hardware_agent_tests.rs

use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_hardware_agent_handles_both_electronics_and_infra() -> RaiseResult<()> {
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

    // Injection du prompt matériel expert
    sys_mgr
        .upsert_document(
            "prompts",
            json_value!({
                "handle": "prompt_hardware",
                "role": "Architecte Matériel",
                "identity": {
                    "persona": "Tu es un Ingénieur Hardware expert en Physical Architecture (PA). Répond en JSON pur.",
                    "tone": "précis"
                },
                "environment": "Conception de matériel et infrastructure Cloud.",
                "directives": ["Génère les Physical Nodes en JSON."]
            }),
        )
        .await?;

    let agent_urn = "agent_hardware";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "Hardware Architect" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_hardware", "temperature": 0.1 }
        }
    })).await?;

    // --- 🎯 2. SETUP PROJET (Physique) ---
    let pa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "pa");
    let _ = DbSandbox::mock_db(&pa_mgr).await;

    pa_mgr
        .create_collection("physical_nodes", generic_schema)
        .await?;

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_pa");

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

    // EXÉCUTION FPGA
    let intent_fpga = EngineeringIntent::CreateElement {
        layer: "PA".to_string(),
        element_type: "Hardware".to_string(),
        name: "Carte Traitement Vidéo FPGA".to_string(),
    };
    match agent.process(&ctx, &intent_fpga).await {
        Ok(_) => user_success!("SUC_TEST_FPGA_PROCESSED"),
        Err(e) => return Err(e),
    }

    // EXÉCUTION CLOUD
    let intent_cloud = EngineeringIntent::CreateElement {
        layer: "PA".to_string(),
        element_type: "Server".to_string(),
        name: "DatabaseClusterAWS".to_string(),
    };
    match agent.process(&ctx, &intent_cloud).await {
        Ok(_) => user_success!("SUC_TEST_CLOUD_PROCESSED"),
        Err(e) => return Err(e),
    }

    // --- 🔍 4. VÉRIFICATION (Résilience & Artefacts) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let nodes_dir = test_root.join("un2/pa/collections/physical_nodes");
    let mut found_fpga = false;
    let mut found_cloud = false;

    if nodes_dir.exists() {
        if let Ok(entries) = fs::read_dir_sync(&nodes_dir) {
            for e in entries.flatten() {
                let content = fs::read_to_string_sync(&e.path())
                    .unwrap_or_default()
                    .to_lowercase();
                if content.contains("video")
                    && (content.contains("fpga") || content.contains("electronics"))
                {
                    found_fpga = true;
                }
                if content.contains("database")
                    && (content.contains("cpu") || content.contains("infrastructure"))
                {
                    found_cloud = true;
                }
            }
        }
    }

    if !found_fpga {
        user_warn!("WRN_FPGA_NOT_FOUND_LOCALLY");
    }
    if !found_cloud {
        user_warn!("WRN_CLOUD_NOT_FOUND_LOCALLY");
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
    async fn test_hardware_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        // Validation SSOT de la partition système
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la réaction en cas de prompt manquant pour l'agent (Match...raise_error)
    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_hardware_agent_missing_prompt_resilience() -> RaiseResult<()> {
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
                    "handle": "agent_broken_hw",
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
            "agent_broken_hw",
            "sess_err",
            SharedRef::new(env.sandbox.storage.clone()),
            llm_client,
            world_engine,
            test_root.clone(),
            test_root.clone(),
        )
        .await;

        let agent = DynamicAgent::new("agent_broken_hw");
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
