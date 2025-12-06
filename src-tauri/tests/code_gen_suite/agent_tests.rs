// CORRECTION : On importe "init_env" qui est défini dans le mod.rs local de cette suite
use crate::common::init_env;
use genaptitude::ai::agents::software_agent::SoftwareAgent;
use genaptitude::ai::agents::{Agent, EngineeringIntent};
use genaptitude::json_db::collections::manager::CollectionsManager;
use serde_json::json;
use std::fs;

#[tokio::test]
#[ignore] // Lent (IA + FileSystem)
async fn test_full_hybrid_generation_flow() {
    // Utilisation de la bonne fonction d'init
    let env = init_env();

    // Pré-requis : Docker
    if !env.client.ping_local().await {
        println!("⚠️ SKIPPED: Docker requis.");
        return;
    }

    // 1. Préparation : On insère manuellement un acteur en base
    let mgr = CollectionsManager::new(&env.storage, "un2", "_system");
    mgr.create_collection("actors", None).unwrap();

    let actor_doc = json!({
        "id": "uuid-hybrid-test",
        "name": "Controleur De Vol",
        "description": "Calcule la trajectoire et ajuste les ailerons.",
        "@type": "oa:OperationalActor"
    });
    mgr.insert_raw("actors", &actor_doc).unwrap();

    // 2. L'Agent entre en scène
    // On lui passe la racine de l'environnement temporaire
    let agent = SoftwareAgent::new(
        env.client.clone(),
        env.storage.clone(),
        env.root_dir.path().to_path_buf(),
    );

    let intent = EngineeringIntent::GenerateCode {
        language: "Rust".to_string(),
        filename: "ControleurDeVol.rs".to_string(),
        context: "Il doit calculer la trajectoire parabolique.".to_string(),
    };

    // 3. Exécution
    let result = agent.process(&intent).await;
    assert!(result.is_ok(), "L'agent a échoué : {:?}", result.err());
    println!("{}", result.unwrap().unwrap());

    // 4. Vérification Physique
    let target_file = env.output_path.join("ControleurDeVol.rs");

    assert!(target_file.exists(), "Le fichier généré est introuvable");

    let content = fs::read_to_string(target_file).unwrap();

    // A. La structure (Symbolique) doit être là
    assert!(content.contains("pub struct ControleurDeVol"));

    // B. La logique (IA) doit avoir remplacé le marqueur
    assert!(
        !content.contains("// AI_INJECTION_POINT"),
        "Le marqueur aurait dû être remplacé"
    );
    assert!(
        content.contains("trajectoire") || content.contains("calcul"),
        "L'IA aurait dû ajouter de la logique"
    );
}
