// FICHIER : src-tauri/tests/code_gen_suite/rust_tests.rs

use crate::common::init_ai_test_env; // REVERSION : Retour à l'import fonctionnel depuis common
use raise::code_generator::{CodeGeneratorService, TargetLanguage};
use serde_json::json;
use std::fs;

#[tokio::test] // CORRECTION : Passage en test asynchrone pour supporter .await
async fn test_rust_skeleton_generation() {
    // CORRECTION E0609 : init_ai_test_env() est désormais asynchrone.
    // On doit l'attendre pour obtenir l'objet AiTestEnv concret.
    let env = init_ai_test_env().await;

    // On utilise le dossier temporaire de l'environnement comme sortie
    let service = CodeGeneratorService::new(env._tmp_dir.path().to_path_buf());

    // 1. Donnée Mock (Acteur)
    // Note : On utilise "type" pour matcher la sérialisation interne
    let actor = json!({
        "id": "uuid-test-pure",
        "name": "Moteur Physique",
        "description": "Simule la gravité.",
        "type": "oa:OperationalActor"
    });

    // 2. Génération
    let paths = service
        .generate_for_element(&actor, TargetLanguage::Rust)
        .expect("La génération doit réussir");

    // 3. Vérifications
    assert_eq!(paths.len(), 1);
    let file_path = &paths[0];

    assert!(file_path.exists(), "Le fichier généré doit exister");

    // Validation du nom de fichier (PascalCase actuel)
    let filename = file_path.file_name().unwrap().to_str().unwrap();
    assert_eq!(
        filename, "MoteurPhysique.rs",
        "Le nom du fichier généré doit correspondre (PascalCase actuel)"
    );

    let content = fs::read_to_string(file_path).unwrap();

    // Validation du contenu (PascalCase)
    assert!(
        content.contains("pub struct MoteurPhysique"),
        "La structure Rust doit être en PascalCase"
    );

    // Note : L'assertion sur AI_INJECTION_POINT reste commentée car non implémentée
}
