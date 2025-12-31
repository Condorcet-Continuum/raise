use crate::common::init_ai_test_env;
// On importe uniquement ce dont on a besoin
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{business_agent::BusinessAgent, Agent, AgentContext};
use raise::ai::llm::client::LlmClient;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn test_business_agent_generates_oa_entities() {
    dotenvy::dotenv().ok();
    let env = init_ai_test_env();

    let api_key = std::env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let local_url =
        std::env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    // Config flexible du modèle
    let model_name = std::env::var("RAISE_MODEL_NAME")
        .map(|s| s.trim().replace("\"", "").to_string())
        .ok();

    // Skip si pas d'IA
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
    // On construit le chemin proprement avec .join() pour éviter les erreurs de séparateur
    let capabilities_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("capabilities");

    // On laisse le temps au système de fichiers (surtout sur Docker/CI)
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    println!("📂 Vérification dans : {:?}", capabilities_dir);

    let mut found_cap = false;
    if capabilities_dir.exists() {
        for entry in std::fs::read_dir(&capabilities_dir).unwrap() {
            if let Ok(e) = entry {
                let content = std::fs::read_to_string(e.path())
                    .unwrap_or_default()
                    .to_lowercase();

                // CORRECTION : Recherche insensible à la casse (instruction vs Instruction)
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
        for entry in std::fs::read_dir(&actors_dir).unwrap() {
            if let Ok(e) = entry {
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
    }
    assert!(
        found_analyst,
        "L'acteur 'Analyste Risque' n'a pas été créé."
    );
    assert!(found_advisor, "L'acteur 'Conseiller' n'a pas été créé.");
}
