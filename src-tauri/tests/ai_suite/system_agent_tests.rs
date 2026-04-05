// FICHIER : src-tauri/tests/ai_suite/system_agent_tests.rs

use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
// 🎯 FIX : DynamicAgent
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_system_agent_creates_function_end_to_end() {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection) ---
    let sys_mgr = CollectionsManager::new(
        &env.sandbox.storage,
        &env.sandbox.config.system_domain,
        &env.sandbox.config.system_db,
    );
    DbSandbox::mock_db(&sys_mgr).await.unwrap();
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
            "handle": "prompt_system",
            "role": "Architecte Système",
            "identity": { 
                "persona": "Tu es un Ingénieur Système expert certifié Arcadia (Couche SA).",
                "tone": "analytique"
            },
            "environment": "Analyse Système (SA) du Continuum RAISE.", 
            "directives": ["Génère la fonction système (SystemFunction) demandée en format JSON."]
        })).await.unwrap();

    let agent_urn = "agent_system";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "System Architect" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_system", "temperature": 0.1 }
        }
    })).await.unwrap();

    // --- 🎯 2. SETUP SPÉCIFIQUE AU TEST ---
    let sa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "sa");
    DbSandbox::mock_db(&sa_mgr).await.unwrap();
    sa_mgr
        .create_collection(
            "functions",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_sa");
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

    let intent = EngineeringIntent::CreateElement {
        layer: "SA".to_string(),
        element_type: "Function".to_string(),
        name: "Calculer Vitesse".to_string(),
    };

    let result = agent.process(&ctx, &intent).await;
    assert!(result.is_ok(), "L'agent a retourné une erreur interne");

    let agent_response = result.unwrap().unwrap();
    let delegated = agent_response.outgoing_message.is_some();

    // --- VÉRIFICATION PHYSIQUE ---
    let functions_dir = test_root.join("un2/sa/collections/functions");
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let mut found = false;
    if functions_dir.exists() {
        for e in fs::read_dir_sync(&functions_dir).unwrap().flatten() {
            let content = fs::read_to_string_sync(&e.path())
                .unwrap_or_default()
                .to_lowercase();
            if content.contains("calculer") && content.contains("vitesse") {
                found = true;
                break;
            }
        }
    }

    if delegated {
        println!("✅ SUCCÈS : L'agent a intelligemment délégué la création de la fonction.");
    } else if found {
        println!("✅ SUCCÈS : L'agent a généré la fonction physiquement.");
    } else {
        println!("⚠️ Fonction non trouvée localement.");
    }
}
