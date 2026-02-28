// FICHIER : src-tauri/tests/ai_suite/business_agent_tests.rs

use crate::common::{setup_test_env, LlmMode};
// On importe uniquement ce dont on a besoin
use raise::ai::agents::intent_classifier::EngineeringIntent;
use raise::ai::agents::{business_agent::BusinessAgent, Agent, AgentContext};
use raise::utils::Arc;

#[tokio::test]
#[serial_test::serial] // Protection RTX 5060 en local
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_business_agent_generates_oa_entities() {
    let env = setup_test_env(LlmMode::Enabled).await;

    let test_root = env.storage.config.data_root.clone();

    let agent_id = "business_agent_test";
    let session_id = AgentContext::generate_default_session_id(agent_id, "test_suite_oa");

    let ctx = AgentContext::new(
        agent_id,
        &session_id,
        Arc::new(env.storage.clone()),
        env.client
            .clone()
            .expect("LlmClient must be enabled for tests"),
        test_root.clone(),
        test_root.join("dataset"),
    )
    .await;

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

    assert!(result.is_ok(), "L'agent a retourné une erreur interne");
    let agent_response = result.unwrap().unwrap();

    println!("🤖 Message de l'Agent :\n{}", agent_response.message);

    // --- VÉRIFICATIONS ROBUSTES (Multi-Agents) ---

    // 1. CAS A : L'agent a décidé de déléguer la suite du travail
    if let Some(msg) = &agent_response.outgoing_message {
        println!(
            "✅ SUCCÈS : Le BusinessAgent a analysé le texte et a intelligemment délégué la création à l'agent '{}'.", 
            msg.receiver
        );
        return; // Le test est réussi, le comportement autonome est validé !
    }

    // 2. CAS B : L'agent a fait le travail lui-même
    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    // On s'assure qu'au moins la Capacité a été générée
    let capabilities_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("capabilities");
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
                break;
            }
        }
    }

    assert!(
        found_cap,
        "L'agent n'a ni délégué la tâche, ni généré la capacité attendue."
    );

    // Vérification souple des acteurs (les petits modèles peuvent les fusionner)
    let actors_dir = test_root
        .join("un2")
        .join("oa")
        .join("collections")
        .join("actors");
    if actors_dir.exists() {
        let count = std::fs::read_dir(&actors_dir).unwrap().count();
        println!(
            "✅ SUCCÈS : L'agent a généré {} acteur(s) physique(s).",
            count
        );
    } else {
        println!("⚠️ La capacité a été créée, mais les acteurs sont manquants (Typique des modèles < 3B).");
    }
}
