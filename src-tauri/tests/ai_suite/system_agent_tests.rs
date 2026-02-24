// FICHIER : src-tauri/tests/ai_suite/system_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{system_agent::SystemAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[ignore]
async fn test_system_agent_creates_function_end_to_end() {
    // CORRECTION E0609 : init_ai_test_env() est désormais asynchrone.
    // On doit l'attendre pour accéder aux champs client et storage.
    let env = setup_test_env(LlmMode::Enabled).await;

    let test_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Injection agent_id + session_id
    let agent_id = "system_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_sa");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for BusinessAgent tests"),
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

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
        for e in std::fs::read_dir(&functions_dir).unwrap().flatten() {
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
    assert!(
        found,
        "La SystemFunction 'Calculer Vitesse' n'a pas été trouvée dans sa/functions."
    );
}
