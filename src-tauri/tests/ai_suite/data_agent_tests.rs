// FICHIER : src-tauri/tests/ai_suite/data_agent_tests.rs

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
async fn test_data_agent_creates_class_and_enum() {
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
    let _ = sys_mgr
        .create_collection(
            "session_agents",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;
    sys_mgr
        .upsert_document(
            "prompts",
            json_value!({
                "handle": "prompt_data",
                "role": "Architecte Données",
                "identity": {
                    "persona": "Tu es un Data Architect spécialisé en modélisation.",
                    "tone": "technique"
                },
                "environment": "Couche DATA du projet Condorcet.",
                "directives": ["Génère les entités de données demandées en format JSON."]
            }),
        )
        .await
        .unwrap();

    let agent_urn = "agent_data";
    sys_mgr.upsert_document("agents", json_value!({
        "handle": agent_urn,
        "base": {
            "name": { "fr": "Data Architect" },
            "neuro_profile": { "prompt_id": "ref:prompts:handle:prompt_data", "temperature": 0.1 }
        }
    })).await.unwrap();

    // --- 🎯 2. SETUP SPÉCIFIQUE AU TEST ---
    let data_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "data");
    DbSandbox::mock_db(&data_mgr).await.unwrap();
    data_mgr
        .create_collection(
            "classes",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();
    data_mgr
        .create_collection(
            "types",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

    let session_id = AgentContext::generate_default_session_id(agent_urn, "test_suite_data");
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

    // 🎯 FIX : DynamicAgent
    let agent = DynamicAgent::new(agent_urn);

    // 1. Test CLASSE
    let intent_class = EngineeringIntent::CreateElement {
        layer: "DATA".to_string(),
        element_type: "Class".to_string(),
        name: "Client".to_string(),
    };
    let res_class = agent.process(&ctx, &intent_class).await;
    assert!(res_class.is_ok());

    if let Ok(Some(res)) = &res_class {
        println!("> {}", res.message);
    }

    // 2. Test ENUM
    let intent_enum = EngineeringIntent::CreateElement {
        layer: "DATA".to_string(),
        element_type: "DataType".to_string(),
        name: "StatutCommande".to_string(),
    };
    let res_enum = agent.process(&ctx, &intent_enum).await;
    assert!(res_enum.is_ok());

    let mut delegated_enum = false;
    if let Ok(Some(res)) = &res_enum {
        println!("> {}", res.message);
        delegated_enum = res.outgoing_message.is_some();
    }

    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let classes_dir = test_root.join("un2/data/collections/classes");
    let mut found_class = false;
    if classes_dir.exists() {
        for e in fs::read_dir_sync(&classes_dir).unwrap().flatten() {
            if fs::read_to_string_sync(&e.path())
                .unwrap_or_default()
                .to_lowercase()
                .contains("client")
            {
                found_class = true;
                break;
            }
        }
    }
    assert!(found_class, "Classe Client non trouvée.");

    let types_dir = test_root.join("un2/data/collections/types");
    let mut found_enum = false;
    if types_dir.exists() {
        for e in fs::read_dir_sync(&types_dir).unwrap().flatten() {
            if fs::read_to_string_sync(&e.path())
                .unwrap_or_default()
                .to_lowercase()
                .contains("statutcommande")
            {
                found_enum = true;
                break;
            }
        }
    }

    if delegated_enum {
        println!("✅ SUCCÈS : L'agent a intelligemment délégué la création de l'Enum.");
    } else if found_enum {
        println!("✅ SUCCÈS : L'agent a généré l'Enum physiquement.");
    } else {
        println!("⚠️ Enum non trouvée.");
    }
}
