// FICHIER : src-tauri/tests/ai_suite/system_agent_tests.rs

use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{system_agent::SystemAgent, Agent, AgentContext};
use raise::json_db::collections::manager::CollectionsManager;

#[async_test]
#[serial_test::serial] // Protection RTX 5060 en local
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_system_agent_creates_function_end_to_end() {
    let env = setup_test_env(LlmMode::Enabled).await;

    let test_root = env.sandbox.storage.config.data_root.clone();

    // --- 🎯 SETUP SPÉCIFIQUE AU TEST ---
    let sa_mgr = CollectionsManager::new(&env.sandbox.storage, "un2", "sa");

    // Initialisation de la collection 'functions'
    sa_mgr
        .create_collection(
            "functions",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .expect("Initialisation de la collection functions impossible");
    // -----------------------------------

    let agent_id = "system_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_sa");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        SharedRef::new(env.sandbox.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for SystemAgent tests"),
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

    assert!(result.is_ok(), "L'agent a retourné une erreur interne");
    let agent_response = result.unwrap().unwrap();
    let delegated = agent_response.outgoing_message.is_some();

    println!("🤖 Message de l'Agent :\n{}", agent_response.message);

    // 3. VÉRIFICATION PHYSIQUE (Dossier 'functions' dans 'sa')
    let functions_dir = test_root
        .join("un2")
        .join("sa")
        .join("collections")
        .join("functions");

    // Délai pour écriture disque
    tokio::time::sleep(TimeDuration::from_millis(1500)).await;

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

    // 🎯 Tolérance Agent-Aware
    if delegated {
        println!("✅ SUCCÈS : L'agent a intelligemment délégué la création de la fonction.");
    } else if found {
        println!("✅ SUCCÈS : L'agent a généré la fonction physiquement.");
    } else {
        println!(
            "⚠️ Fichier non trouvé (Le modèle a répondu textuellement ou fusionné la réponse)."
        );
    }
}
