// FICHIER : src-tauri/tests/ai_suite/business_agent_tests.rs

use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
// 🎯 FIX : On remplace BusinessAgent par DynamicAgent
use raise::ai::agents::{dynamic_agent::DynamicAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_business_agent_generates_oa_entities() {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 1. SETUP SYSTEM (Injection de l'agent dynamique) ---
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
        "_id": "ref:prompts:handle:prompt_business",
        "role": "Analyste Métier",
        "identity": { "persona": "Tu es un Business Analyst expert en Operational Analysis (OA)." },
        "directives": ["Génère les entités métier demandées en format JSON."]
    })).await.unwrap();

    let agent_urn = "ref:agents:handle:agent_business";
    sys_mgr.upsert_document("agents", json_value!({
        "_id": agent_urn,
        "base": {
            "name": { "fr": "Business Analyst" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_business", "temperature": 0.1 }
        }
    })).await.unwrap();

    // --- 🎯 2. SETUP SPÉCIFIQUE AU TEST (OA) ---
    let oa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "oa");
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

    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_oa");
    use candle_nn::VarMap;
    let wm_config = raise::utils::data::config::WorldModelConfig::default();
    let world_engine = SharedRef::new(
        raise::ai::world_model::NeuroSymbolicEngine::new(wm_config, VarMap::new()).unwrap(),
    );

    let ctx = AgentContext::new(
        agent_urn,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for tests"),
        world_engine,
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    // 🎯 FIX : Utilisation du DynamicAgent
    let agent = DynamicAgent::new(agent_urn);

    let intent = EngineeringIntent::DefineBusinessUseCase {
        domain: "Banque".to_string(),
        process_name: "Instruction Crédit Immo".to_string(),
        description: "Je souhaite modéliser le processus d'instruction d'un crédit immobilier. \
                      Un Client dépose une demande. Un Conseiller vérifie les pièces. \
                      Un Analyste Risque valide le dossier."
            .to_string(),
    };

    println!("👔 Lancement du Business Agent (Dynamique)...");
    let result = agent.process(&ctx, &intent).await;

    assert!(result.is_ok(), "L'agent a retourné une erreur interne");
    let agent_response = result.unwrap().unwrap();

    println!("🤖 Message de l'Agent :\n{}", agent_response.message);

    if let Some(_msg) = &agent_response.outgoing_message {
        println!("✅ SUCCÈS : Le BusinessAgent a intelligemment délégué la création.");
        return;
    }

    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let capabilities_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("capabilities");
    let mut found_cap = false;

    if capabilities_dir.exists() {
        for e in fs::read_dir_sync(&capabilities_dir).unwrap().flatten() {
            let content = fs::read_to_string_sync(&e.path())
                .unwrap_or_default()
                .to_lowercase();
            if content.contains("crédit")
                || content.contains("instruction")
                || content.contains("immo")
            {
                found_cap = true;
                break;
            }
        }
    }

    assert!(
        found_cap,
        "L'agent n'a ni délégué la tâche, ni généré la capacité attendue."
    );

    let actors_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("actors");
    if actors_dir.exists() {
        let count = fs::read_dir_sync(&actors_dir).unwrap().count();
        println!(
            "✅ SUCCÈS : L'agent a généré {} acteur(s) physique(s).",
            count
        );
    }
}
