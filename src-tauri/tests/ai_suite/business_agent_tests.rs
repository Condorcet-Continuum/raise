// FICHIER : src-tauri/tests/ai_suite/business_agent_tests.rs

use crate::common::setup_test_env;
// On importe uniquement ce dont on a besoin
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{business_agent::BusinessAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[ignore]
async fn test_business_agent_generates_oa_entities() {
    let env = setup_test_env().await;

    let test_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Fournit les 6 arguments requis incluant l'ID agent et session
    let agent_id = "business_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_oa");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client.clone(),
        test_root.clone(),
        test_root.join("dataset"),
    );

    let agent = BusinessAgent::new();

    // INTENTION MÉTIER
    let intent = EngineeringIntent::DefineBusinessUseCase {
        domain: "Banque".to_string(),
        process_name: "Instruction Crédit Immo".to_string(),
        description: "Je souhaite modéliser le processus d'instruction d'un crédit immobilier. \
                      Un Client dépose une demande. Un Conseiller vérifie les pièces. \
                      Un Analyste Risque valide le dossier."
            .to_string(),
    };

    println!("👔 Lancement du Business Agent...");
    let result = agent.process(&ctx, &intent).await;

    if let Err(e) = &result {
        println!("❌ Erreur : {}", e);
    }
    assert!(result.is_ok());
    println!("{}", result.unwrap().unwrap());

    // --- VÉRIFICATIONS (Robustes) ---

    // 1. CAPACITÉS (OA)
    let capabilities_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("capabilities");

    // On laisse le temps au système de fichiers
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    println!("📂 Vérification dans : {:?}", capabilities_dir);

    let mut found_cap = false;
    if capabilities_dir.exists() {
        for e in std::fs::read_dir(&capabilities_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();

            if content.contains("crédit")
                || content.contains("instruction")
                || content.contains("immo")
            {
                found_cap = true;
                println!("✅ Capacité trouvée : {:?}", e.file_name());
                break;
            } else {
                println!(
                    "   ℹ️ Fichier ignoré (contenu non matché) : {:?}",
                    e.file_name()
                );
            }
        }
    } else {
        println!("❌ Le dossier 'capabilities' n'existe pas !");
    }
    assert!(
        found_cap,
        "Aucune OperationalCapability correspondante trouvée dans OA."
    );

    // 2. ACTEURS (OA)
    let actors_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("actors");
    let mut found_analyst = false;
    let mut found_advisor = false;

    if actors_dir.exists() {
        for e in std::fs::read_dir(&actors_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();
            if content.contains("analyste") {
                found_analyst = true;
            }
            if content.contains("conseiller") {
                found_advisor = true;
            }
        }
    }
    assert!(
        found_analyst,
        "L'acteur 'Analyste Risque' n'a pas été créé."
    );
    assert!(found_advisor, "L'acteur 'Conseiller' n'a pas été créé.");
}
