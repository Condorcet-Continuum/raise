// FICHIER : src-tauri/tests/ai_suite/epbs_agent_tests.rs

use crate::common::setup_test_env;
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{epbs_agent::EpbsAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[ignore]
async fn test_epbs_agent_creates_configuration_item() {
    // CORRECTION E0609 : init_ai_test_env() est désormais asynchrone.
    // On doit utiliser .await pour récupérer l'objet AiTestEnv.
    let env = setup_test_env().await;

    let test_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Injection agent_id + session_id
    let agent_id = "epbs_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_epbs");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client.clone(),
        test_root.clone(),
        test_root.join("dataset"),
    );

    let agent = EpbsAgent::new();

    // SCÉNARIO : Créer un "Serveur Rack"
    let intent = EngineeringIntent::CreateElement {
        layer: "EPBS".to_string(),
        element_type: "COTS".to_string(),
        name: "Rack Server Dell R750".to_string(),
    };

    println!("📦 Lancement EPBS Agent...");
    let result = agent.process(&ctx, &intent).await;
    assert!(result.is_ok());
    if let Ok(Some(res)) = &result {
        println!("{}", res);
    }

    // VÉRIFICATION
    let items_dir = test_root
        .join("un2")
        .join("epbs")
        .join("collections")
        .join("configuration_items");
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let mut found = false;
    if items_dir.exists() {
        for e in std::fs::read_dir(&items_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path()).unwrap_or_default();

            // Debug : Affiche ce qu'on a trouvé pour comprendre pourquoi ça match pas
            println!("📄 Analyse fichier : {:?}", e.file_name());
            println!(
                "   Contenu partiel : {:.100}...",
                content.replace("\n", " ")
            );

            if content.contains("partNumber") && content.contains("Rack Server") {
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
