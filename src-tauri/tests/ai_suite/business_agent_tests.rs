// FICHIER : src-tauri/tests/ai_suite/business_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*;
use raise::utils::testing::DbSandbox;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_business_agent_generates_oa_entities() {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection) ---
    let sys_mgr = CollectionsManager::new(
        &env.sandbox.storage,
        &env.sandbox.config.system_domain,
        &env.sandbox.config.system_db,
    );
    DbSandbox::mock_db(&sys_mgr).await.unwrap();

    for coll in &[
        "prompts",
        "agents",
        "session_agents",
        "configs",
        "databases",
    ] {
        let _ = sys_mgr
            .create_collection(
                coll,
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;
    }

    // Déclarations vitales
    sys_mgr
        .upsert_document(
            "databases",
            json_value!({ "handle": "oa", "domain": "un2" }),
        )
        .await
        .unwrap();
    sys_mgr
        .upsert_document(
            "configs",
            json_value!({
                "handle": "ontological_mapping",
                "mappings": {
                    "OperationalCapability": { "layer": "oa", "collection": "capabilities" },
                    "OperationalActor": { "layer": "oa", "collection": "actors" }
                }
            }),
        )
        .await
        .unwrap();

    // Prompt strict
    sys_mgr.upsert_document("prompts", json_value!({
        "handle": "prompt_business",
        "role": "Analyste Métier",
        "identity": { "persona": "Expert Arcadia. Répond en JSON pur.", "tone": "froid" },
        "environment": "Système RAISE.",
        "directives": ["Génère un TABLEAU JSON avec: '_id', 'name', 'type' (OperationalActor/OperationalCapability), 'layer' (OA)."]
    })).await.unwrap();

    sys_mgr.upsert_document("agents", json_value!({
        "handle": "agent_business",
        "base": {
            "name": { "fr": "Business Analyst" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_business", "temperature": 0.0 }
        }
    })).await.unwrap();

    // --- 🎯 2. SETUP PROJECT (Physique) ---
    let oa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "oa");
    DbSandbox::mock_db(&oa_mgr).await.unwrap();
    oa_mgr
        .create_collection(
            "capabilities",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();
    oa_mgr
        .create_collection(
            "actors",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

    // --- 🎯 3. CONTEXTE & EXÉCUTION ---
    let session_id = AgentContext::generate_default_session_id("agent_business", "test_oa");
    use candle_nn::VarMap;
    let wm_config = raise::utils::data::config::WorldModelConfig::default();
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap(),
    );

    let ctx = AgentContext::new(
        "agent_business",
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()), // 🎯 FIX : Enveloppement manuel dans SharedRef (Arc)
        env.client.clone().expect("LlmClient requis"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    let agent = DynamicAgent::new("agent_business");
    let intent = EngineeringIntent::DefineBusinessUseCase {
        domain: "Banque".to_string(),
        process_name: "Crédit".to_string(),
        description: "Un Client dépose un dossier.".to_string(),
    };

    let result = agent.process(&ctx, &intent).await;
    assert!(result.is_ok());

    // --- 🔍 4. VÉRIFICATION (Via Système de fichiers) ---
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    // 🎯 FIX : On utilise le même pattern que hardware_agent_tests.rs
    let cap_dir = test_root.join("un2/oa/collections/capabilities");
    let act_dir = test_root.join("un2/oa/collections/actors");

    let mut found = false;
    if cap_dir.exists() && fs::read_dir_sync(&cap_dir).unwrap().flatten().count() > 0 {
        found = true;
    }

    if !found && act_dir.exists() && fs::read_dir_sync(&act_dir).unwrap().flatten().count() > 0 {
        found = true;
    }

    assert!(found, "L'IA n'a produit aucun fichier dans 'un2/oa'.");
    println!("✅ SUCCÈS : Analyse Opérationnelle validée !");
}
