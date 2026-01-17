// FICHIER : src-tauri/tests/ai_suite/system_agent_tests.rs

use crate::common::init_ai_test_env;
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{system_agent::SystemAgent, Agent, AgentContext};
use raise::ai::llm::client::LlmClient;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn test_system_agent_creates_function_end_to_end() {
    dotenvy::dotenv().ok();

    // CORRECTION E0609 : init_ai_test_env() est désormais asynchrone.
    // On doit l'attendre pour accéder aux champs client et storage.
    let env = init_ai_test_env().await;

    // 1. Config Robuste (Identique aux autres agents)
    let api_key = std::env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let local_url =
        std::env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    // Fallback propre pour le modèle
    let model_name = std::env::var("RAISE_MODEL_NAME")
        .map(|s| s.trim().replace("\"", "").to_string())
        .ok();

    // Skip si pas d'IA disponible
    if !env.client.ping_local().await && api_key.is_empty() {
        println!("⚠️ SKIPPED: Pas d'IA disponible.");
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

    let agent = SystemAgent::new();

    // 2. SCÉNARIO : Création d'une Fonction Système (SA)
    let intent = EngineeringIntent::CreateElement {
        layer: "SA".to_string(),
        element_type: "Function".to_string(),
        name: "Calculer Vitesse".to_string(),
    };

    println!("⚙️ Lancement du System Agent...");
    let result = agent.process(&ctx, &intent).await;

    if let Err(e) = &result {
        println!("❌ Erreur : {}", e);
    }
    assert!(result.is_ok());
    println!("{}", result.unwrap().unwrap());

    // 3. VÉRIFICATION PHYSIQUE (Dossier 'functions' dans 'sa')
    let functions_dir = test_root
        .join("un2")
        .join("sa")
        .join("collections")
        .join("functions");

    // Délai pour écriture disque
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    println!("📂 Vérification dans : {:?}", functions_dir);

    let mut found = false;
    if functions_dir.exists() {
        for entry in std::fs::read_dir(&functions_dir).unwrap() {
            if let Ok(e) = entry {
                let content = std::fs::read_to_string(e.path())
                    .unwrap_or_default()
                    .to_lowercase();

                // Recherche insensible à la casse
                if content.contains("calculer") && content.contains("vitesse") {
                    found = true;
                    println!("✅ Fonction Système trouvée : {:?}", e.file_name());
                    break;
                }
            }
        }
    }
    assert!(
        found,
        "La SystemFunction 'Calculer Vitesse' n'a pas été trouvée dans sa/functions."
    );
}
