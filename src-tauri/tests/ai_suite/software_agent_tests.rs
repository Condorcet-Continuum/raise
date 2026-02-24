// FICHIER : src-tauri/tests/ai_suite/software_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use raise::ai::agents::{software_agent::SoftwareAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[ignore]
async fn test_software_agent_creates_component_end_to_end() {
    let env = setup_test_env(LlmMode::Enabled).await;

    // --- CONTEXTE ---
    let test_data_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Injection agent_id + session_id
    let agent_id = "software_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_la");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for BusinessAgent tests"),
        test_data_root.clone(),
        test_data_root.join("dataset"),
    )
    .await;

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
    // CORRECTION E0609 : .await ajouté ici également.
    let env = setup_test_env(LlmMode::Enabled).await;
    let classifier = IntentClassifier::new(
        env.client
            .clone()
            .expect("LlmClient est requis pour l'IntentClassifier (utilisez LlmMode::Enabled)"),
    );

    // --- CORRECTION : Prompt "Anti-Markdown" ---
    let input = "Instruction: Analyse cette demande et retourne le JSON strict. \
                 IMPORTANT: Ne jamais échapper les underscores (pas de backslash '\\' avant '_'). \
                 Exemple valide: 'create_element'. Exemple invalide: 'create\\_element'. \n\
                 Demande: Crée une fonction système nommée 'DémarrerMoteur'";

    let intent = classifier.classify(input).await;
    println!("➤ Result Intent: {:?}", intent);

    match intent {
        EngineeringIntent::CreateElement { name, .. } => {
            // Nettoyage au cas où
            let clean_name = name.replace("'", "").replace("\"", "");
            assert!(
                clean_name.to_lowercase().contains("demarrermoteur")
                    || clean_name.to_lowercase().contains("démarrermoteur"),
                "Nom incorrect. Reçu: '{}'",
                name
            );
        }
        _ => panic!("Classification échouée. Reçu: {:?}", intent),
    }
}
