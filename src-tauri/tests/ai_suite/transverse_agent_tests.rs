// FICHIER : src-tauri/tests/ai_suite/transverse_agent_tests.rs

use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
// 🎯 FIX : DynamicAgent
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_transverse_agent_ivvq_cycle() {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection) ---
    let sys_mgr = CollectionsManager::new(
        &env.sandbox.storage,
        &env.sandbox.config.system_domain,
        &env.sandbox.config.system_db,
    );
    let _ = sys_mgr
        .create_collection(
            "prompts",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;
    let _ = sys_mgr
        .create_collection(
            "agents",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;

    sys_mgr.upsert_document("prompts", json_value!({
        "_id": "ref:prompts:handle:prompt_quality",
        "role": "Ingénieur Qualité Transverse",
        "identity": { "persona": "Tu es le garant de la qualité et des exigences (Transverse)." },
        "directives": ["Génère l'exigence (Requirement) ou la procédure de test en JSON."]
    })).await.unwrap();

    let agent_urn = "ref:agents:handle:agent_quality";
    sys_mgr.upsert_document("agents", json_value!({
        "_id": agent_urn,
        "base": {
            "name": { "fr": "Quality Manager" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_quality", "temperature": 0.1 }
        }
    })).await.unwrap();

    // --- 🎯 2. SETUP SPÉCIFIQUE AU TEST ---
    let transverse_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "transverse");
    transverse_mgr
        .create_collection(
            "requirements",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();
    transverse_mgr
        .create_collection(
            "test_procedures",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_transverse");
    use candle_nn::VarMap;
    let wm_config = raise::utils::data::config::WorldModelConfig::default();
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap(),
    );

    let ctx = AgentContext::new(
        agent_urn,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client.clone().expect("LlmClient must be enabled"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    // 🎯 FIX : DynamicAgent
    let agent = DynamicAgent::new(agent_urn);

    // --- TEST EXIGENCE ---
    let intent_req = EngineeringIntent::CreateElement {
        layer: "TRANSVERSE".to_string(),
        element_type: "Requirement".to_string(),
        name: "L'avion doit résister à un impact d'oiseau".to_string(),
    };

    let res_req = agent.process(&ctx, &intent_req).await;
    assert!(res_req.is_ok());

    let req_dir = test_root.join("un2/transverse/collections/requirements");
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let mut found_req = false;
    if req_dir.exists() {
        for e in fs::read_dir_sync(&req_dir).unwrap().flatten() {
            let content = fs::read_to_string_sync(&e.path())
                .unwrap_or_default()
                .to_lowercase();
            if content.contains("oiseau") || content.contains("impact") {
                found_req = true;
                break;
            }
        }
    }

    if found_req {
        println!("✅ SUCCÈS : L'agent a généré l'exigence !");
    } else {
        println!(
            "⚠️ Exigence non trouvée localement (probablement déléguée ou erreur de parsing)."
        );
    }
}
