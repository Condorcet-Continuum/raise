use crate::common::init_ai_test_env;
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{epbs_agent::EpbsAgent, Agent, AgentContext};
use raise::ai::llm::client::LlmClient;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn test_epbs_agent_creates_configuration_item() {
    dotenvy::dotenv().ok();
    let env = init_ai_test_env();

    let api_key = std::env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let local_url =
        std::env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let model_name = std::env::var("RAISE_MODEL_NAME").ok();

    if !env.client.ping_local().await && api_key.is_empty() {
        return;
    }

    let client = LlmClient::new(&local_url, &api_key, model_name);
    let test_root = env.storage.config.data_root.clone();
    let ctx = AgentContext::new(
        Arc::new(env.storage.clone()),
        client.clone(),
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
        for entry in std::fs::read_dir(&items_dir).unwrap() {
            if let Ok(e) = entry {
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
    }
    assert!(
        found,
        "Le Configuration Item n'a pas été créé correctement (voir logs)."
    );
}
