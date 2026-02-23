// FICHIER : src-tauri/tests/ai_suite/hardware_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{hardware_agent::HardwareAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[ignore]
async fn test_hardware_agent_handles_both_electronics_and_infra() {
    // CORRECTION E0609 : init_ai_test_env() est désormais asynchrone suite à la migration
    // vers le moteur de stockage asynchrone. On doit l'attendre pour obtenir l'environnement.
    let env = setup_test_env(LlmMode::Enabled).await;

    let test_root = env.storage.config.data_root.clone();

    // CORRECTION E0061 : Injection agent_id + session_id
    let agent_id = "hardware_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_pa");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for BusinessAgent tests"),
        test_root.clone(),
        test_root.join("dataset"),
    );

    let agent = HardwareAgent::new();

    // --- OBJECTIF 1 : HARDWARE PUR (FPGA) ---
    println!("🔧 Test 1 : Création FPGA...");
    let intent_fpga = EngineeringIntent::CreateElement {
        layer: "PA".to_string(),
        element_type: "FPGA".to_string(),
        name: "VideoProcessingUnit".to_string(),
    };
    let res_fpga = agent.process(&ctx, &intent_fpga).await;
    assert!(res_fpga.is_ok());
    println!("   > {}", res_fpga.unwrap().unwrap());

    // --- OBJECTIF 2 : INFRASTRUCTURE (Cloud) ---
    println!("☁️ Test 2 : Création Serveur Cloud...");
    let intent_cloud = EngineeringIntent::CreateElement {
        layer: "PA".to_string(),
        element_type: "Server".to_string(),
        name: "DatabaseClusterAWS".to_string(),
    };
    let res_cloud = agent.process(&ctx, &intent_cloud).await;
    assert!(res_cloud.is_ok());
    println!("   > {}", res_cloud.unwrap().unwrap());

    // --- VÉRIFICATION PHYSIQUE ---
    let nodes_dir = test_root
        .join("un2")
        .join("pa")
        .join("collections")
        .join("physical_nodes");
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    let mut found_fpga = false;
    let mut found_cloud = false;

    if nodes_dir.exists() {
        for e in std::fs::read_dir(&nodes_dir).unwrap().flatten() {
            let content = std::fs::read_to_string(e.path())
                .unwrap_or_default()
                .to_lowercase();

            // Vérifie FPGA (Nature: Electronics)
            if content.contains("video")
                && (content.contains("fpga") || content.contains("electronics"))
            {
                found_fpga = true;
            }
            // Vérifie Cloud (Nature: Infrastructure)
            if content.contains("database")
                && (content.contains("cpu") || content.contains("infrastructure"))
            {
                found_cloud = true;
            }
        }
    }
    assert!(
        found_fpga,
        "L'élément FPGA n'a pas été trouvé ou mal catégorisé."
    );
    assert!(
        found_cloud,
        "L'élément Cloud n'a pas été trouvé ou mal catégorisé."
    );
}
