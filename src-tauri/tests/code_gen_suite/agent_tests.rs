// FICHIER : src-tauri/tests/code_gen_suite/agent_tests.rs

use crate::common::setup_test_env; // REVERSION : Retour à l'import fonctionnel depuis common
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{software_agent::SoftwareAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[ignore]
async fn test_software_agent_creates_component_end_to_end() {
    // CORRECTION : setup_test_env() est asynchrone, on l'attend pour obtenir AiTestEnv.
    let env = setup_test_env().await;

    // --- CONTEXTE ---
    let test_data_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Injection de l'identité et de la session pour l'isolation
    let agent_id = "software_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_codegen");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client.clone(),
        test_data_root.clone(),
        test_data_root.join("dataset"),
    );

    let agent = SoftwareAgent::new();

    let intent = EngineeringIntent::CreateElement {
        layer: "LA".to_string(),
        element_type: "Component".to_string(),
        name: "TestAuthService".to_string(),
    };

    // --- EXECUTION ---
    let result = agent.process(&ctx, &intent).await;

    if let Err(e) = &result {
        println!("❌ Erreur Agent : {:?}", e);
    }
    assert!(result.is_ok(), "L'agent a planté");

    // --- VERIFICATION ---
    let components_dir = test_data_root
        .join("un2")
        .join("la")
        .join("collections")
        .join("components");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let mut found = false;
    if components_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&components_dir) {
            for e in entries.flatten() {
                let content = std::fs::read_to_string(e.path()).unwrap_or_default();
                if content.contains("TestAuthService") {
                    found = true;
                    break;
                }
            }
        }
    }
    assert!(found, "Fichier JSON non créé.");
}

#[tokio::test]
#[ignore]
async fn test_intent_classification_integration() {
    let env = setup_test_env().await;

    let classifier = IntentClassifier::new(env.client);

    // --- TEST 1 : CREATION ---
    let input = "Crée une fonction système nommée 'DemarrerMoteur'";
    println!("➤ Input 1: {}", input);

    let intent = classifier.classify(input).await;
    println!("➤ Result 1: {:?}", intent);

    match intent {
        EngineeringIntent::CreateElement { name, .. } => {
            let clean_name = name.replace("'", "").replace("\"", "");
            assert!(
                clean_name.to_lowercase().contains("demarrermoteur")
                    || clean_name.to_lowercase().contains("demarrer"),
                "Nom incorrect. Reçu: '{}'",
                name
            );
        }
        _ => panic!("Classification Type 1 échouée. Reçu: {:?}", intent),
    }

    // --- TEST 2 : CODE GEN ---
    let input_code = "Génère le code Rust pour le composant Auth. IMPORTANT: Le JSON DOIT contenir le champ \"filename\": \"auth.rs\".";
    println!("➤ Input 2: {}", input_code);

    let intent_code = classifier.classify(input_code).await;
    println!("➤ Result 2: {:?}", intent_code);

    match intent_code {
        EngineeringIntent::GenerateCode {
            language, filename, ..
        } => {
            assert!(language.to_lowercase().contains("rust"));
            assert!(
                !filename.is_empty(),
                "Filename vide ! L'IA a ignoré l'instruction."
            );
        }
        _ => panic!("Classification Code échouée. Reçu: {:?}", intent_code),
    }
}
