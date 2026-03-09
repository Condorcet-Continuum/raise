use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{epbs_agent::EpbsAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_epbs_agent_creates_configuration_item() {
    let env = setup_test_env(LlmMode::Enabled).await;
    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 SETUP SPÉCIFIQUE AU TEST ---
    // Plus besoin d'amorcer _system ou agent_sessions, mod.rs s'en charge !
    // On prépare juste la base métier locale au test pour guider le LLM.
    let epbs_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "epbs");
    epbs_mgr
        .create_collection(
            "configuration_items",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .expect("Initialisation de la collection métier impossible");
    // -----------------------------------

    let agent_id = "epbs_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_epbs");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for tests"),
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

    let agent = EpbsAgent::new();

    let intent = EngineeringIntent::CreateElement {
        layer: "EPBS".to_string(),
        element_type: "COTS".to_string(),
        name: "Rack Server Dell R750".to_string(),
    };

    println!("📦 Lancement EPBS Agent...");
    let result = agent.process(&ctx, &intent).await;
    assert!(result.is_ok());
    if let Ok(Some(res)) = &result {
        println!("{:?}", res);
    }

    // VÉRIFICATION
    let items_dir = test_root
        .join("un2")
        .join("epbs")
        .join("collections")
        .join("configuration_items");
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

    let mut found = false;
    if items_dir.exists() {
        for e in std::fs::read_dir(&items_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();

            if content.contains("name") && content.contains("Rack Server") {
                found = true;
                println!("✅ CI validé !");
                break;
            }
        }
    }
    assert!(
        found,
        "Le Configuration Item n'a pas été créé correctement (voir logs)."
    );
}
