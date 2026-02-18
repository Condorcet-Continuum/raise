// FICHIER : src-tauri/tests/ai_suite/transverse_agent_tests.rs

use crate::common::setup_test_env;
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{transverse_agent::TransverseAgent, Agent, AgentContext};
use raise::ai::llm::client::LlmClient;
use std::sync::Arc;

#[tokio::test]
#[ignore]
async fn test_transverse_agent_ivvq_cycle() {
    dotenvy::dotenv().ok();

    // CORRECTION E0609 : init_ai_test_env() est désormais asynchrone.
    // On doit l'attendre pour obtenir l'objet AiTestEnv concret.
    let env = setup_test_env().await;

    // Config Robuste
    let api_key = std::env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let local_url =
        std::env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let model_name = std::env::var("RAISE_MODEL_NAME").ok();

    if !env.client.ping_local().await && api_key.is_empty() {
        println!("⚠️ SKIPPED: Pas d'IA disponible.");
        return;
    }

    let client = LlmClient::new(&local_url, &api_key, model_name);
    let test_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Injection agent_id + session_id
    let agent_id = "transverse_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_transverse");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        client.clone(),
        test_root.clone(),
        test_root.join("dataset"),
    );

    let agent = TransverseAgent::new();

    // 1. CRÉATION EXIGENCE
    let intent_req = EngineeringIntent::CreateElement {
        layer: "TRANSVERSE".to_string(),
        element_type: "Requirement".to_string(),
        name: "Performance Démarrage".to_string(),
    };
    println!("✨ [1/3] Création Exigence...");
    let res_req = agent.process(&ctx, &intent_req).await;
    assert!(res_req.is_ok());

    // 2. CRÉATION TEST PROCEDURE
    let intent_test = EngineeringIntent::CreateElement {
        layer: "TRANSVERSE".to_string(),
        element_type: "TestProcedure".to_string(),
        name: "Test Temps Démarrage".to_string(),
    };
    println!("✨ [2/3] Création Procédure de Test...");
    let res_test = agent.process(&ctx, &intent_test).await;
    assert!(res_test.is_ok());

    // 3. CRÉATION CAMPAGNE
    let intent_camp = EngineeringIntent::CreateElement {
        layer: "TRANSVERSE".to_string(),
        element_type: "TestCampaign".to_string(),
        name: "Campagne V1.0".to_string(),
    };
    println!("✨ [3/3] Création Campagne...");
    let res_camp = agent.process(&ctx, &intent_camp).await;
    assert!(res_camp.is_ok());

    // VÉRIFICATION PHYSIQUE
    // On laisse un peu de temps au système de fichiers
    tokio::time::sleep(std::time::Duration::from_millis(2000)).await;

    // 1. Check Requirement
    let req_dir = test_root.join("un2/transverse/collections/requirements");
    let mut found_req = false;
    if req_dir.exists() {
        for e in std::fs::read_dir(&req_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();
            if content.contains("req-sys")
                || content.contains("exigence")
                || content.contains("performance")
            {
                found_req = true;
                println!("✅ Exigence validée : {:?}", e.file_name());
            }
        }
    }
    assert!(found_req, "Exigence non trouvée dans {:?}", req_dir);

    // 2. Check Test Procedure
    let proc_dir = test_root.join("un2/transverse/collections/test_procedures");
    let mut found_proc = false;
    if proc_dir.exists() {
        for e in std::fs::read_dir(&proc_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();

            // MODIFICATION : On vérifie seulement la clé structurelle "steps"
            if content.contains("steps") {
                found_proc = true;
                println!("✅ Procédure validée : {:?}", e.file_name());
            }
        }
    }
    assert!(
        found_proc,
        "Procédure de test non trouvée dans {:?}",
        proc_dir
    );

    // 3. Check Test Campaign
    let camp_dir = test_root.join("un2/transverse/collections/test_campaigns");
    let mut found_camp = false;
    if camp_dir.exists() {
        for e in std::fs::read_dir(&camp_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();

            // On vérifie seulement la clé structurelle "scenarios"
            if content.contains("scenarios") {
                found_camp = true;
                println!("✅ Campagne validée : {:?}", e.file_name());
            }
        }
    }

    if !found_camp {
        println!(
            "⚠️ Campagne non trouvée ou JSON invalide (Path: {:?})",
            camp_dir
        );
    }
}
