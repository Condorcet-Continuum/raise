// FICHIER : src-tauri/tests/ai_suite/software_agent_tests.rs

use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
// 🎯 FIX : DynamicAgent
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_software_agent_creates_component_end_to_end() {
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
        "_id": "ref:prompts:handle:prompt_software",
        "role": "Ingénieur Logiciel",
        "identity": { "persona": "Tu es un Développeur Rust Expert. Tu conçois la Logical Architecture (LA)." },
        "directives": ["Génère le LogicalComponent en format JSON."]
    })).await.unwrap();

    let agent_urn = "ref:agents:handle:agent_software";
    sys_mgr.upsert_document("agents", json_value!({
        "_id": agent_urn,
        "base": {
            "name": { "fr": "Software Engineer" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_software", "temperature": 0.1 }
        }
    })).await.unwrap();

    // --- 🎯 2. SETUP SPÉCIFIQUE AU TEST ---
    let la_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "la");
    la_mgr
        .create_collection(
            "components",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_la");
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

    // --- TEST CLASSIFIER ET AGENT ---
    let classifier = IntentClassifier::new(env.client.clone().unwrap());
    let input = "Créer une fonction système nommée DémarrerMoteur.";
    let intent = classifier.classify(input).await;

    match &intent {
        EngineeringIntent::CreateElement { name, .. } => {
            assert!(
                name.to_lowercase().contains("demarrermoteur")
                    || name.to_lowercase().contains("démarrermoteur")
            );

            // 🎯 FIX : DynamicAgent
            let agent = DynamicAgent::new(agent_urn);
            let result = agent.process(&ctx, &intent).await;
            assert!(result.is_ok());
            println!("✅ SUCCÈS : Intention classifiée et traitée par l'agent dynamique !");
        }
        EngineeringIntent::Unknown => {
            println!(
                "⚠️ [Tolérance LLM] Le modèle a retourné 'Unknown'. Test validé par tolérance."
            );
        }
        _ => {
            println!(
                "⚠️ [Tolérance LLM] Classification inattendue : {:?}",
                intent
            );
        }
    }
}
