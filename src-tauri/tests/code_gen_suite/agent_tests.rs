// FICHIER : src-tauri/tests/code_gen_suite/agent_tests.rs

use crate::common::init_ai_test_env;
use genaptitude::ai::agents::intent_classifier::{EngineeringIntent, IntentClassifier};
use genaptitude::ai::agents::{system_agent::SystemAgent, Agent};

#[tokio::test]
#[ignore]
async fn test_system_agent_creates_actor_end_to_end() {
    let env = init_ai_test_env();

    if !env.client.ping_local().await {
        println!("⚠️ SKIPPED: Docker requis pour le test end-to-end agent.");
        return;
    }

    // L'agent va utiliser CollectionsManager qui a besoin de _system.json (maintenant fourni par init_ai_test_env)
    let agent = SystemAgent::new(env.client.clone(), env.storage.clone());

    let intent = EngineeringIntent::CreateElement {
        layer: "OA".to_string(),
        element_type: "Actor".to_string(), // Correspondra à "actors" via le mapping de l'agent
        name: "TestUnitBot".to_string(),
    };

    let result = agent.process(&intent).await;

    if let Err(e) = &result {
        println!("❌ Erreur Agent : {:?}", e);
    }
    assert!(result.is_ok(), "L'agent a planté");

    let msg = result.unwrap();
    println!("Résultat Agent : {:?}", msg);

    // Vérification physique
    let db_root = env.storage.config.db_root(&env._space, &env._db);
    let actors_dir = db_root.join("collections").join("actors");

    // Le dossier doit exister car create_collection l'a créé (grâce au bootstrap)
    assert!(
        actors_dir.exists(),
        "Le dossier 'actors' doit avoir été créé"
    );

    let mut found = false;
    if let Ok(entries) = std::fs::read_dir(actors_dir) {
        for entry in entries {
            if let Ok(e) = entry {
                let content = std::fs::read_to_string(e.path()).unwrap_or_default();
                if content.contains("TestUnitBot") {
                    found = true;
                    // On vérifie que l'agent a bien généré une description via le LLM
                    assert!(content.contains("description"), "La description IA manque");
                    break;
                }
            }
        }
    }

    assert!(
        found,
        "Le fichier JSON de l'acteur n'a pas été trouvé sur le disque !"
    );
}

#[tokio::test]
#[ignore]
async fn test_intent_classification_integration() {
    let env = init_ai_test_env();

    if !env.client.ping_local().await {
        return;
    }

    let classifier = IntentClassifier::new(env.client.clone());

    let input = "Crée une fonction système nommée 'Démarrer Moteur'";

    let intent = classifier.classify(input).await;

    match intent {
        EngineeringIntent::CreateElement {
            layer,
            element_type,
            name,
        } => {
            assert_eq!(layer, "SA");
            assert_eq!(element_type, "Function");
            assert!(name.contains("Démarrer"));
        }
        _ => panic!("Classification échouée. Reçu: {:?}", intent),
    }
}
