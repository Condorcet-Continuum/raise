// FICHIER : src-tauri/tests/code_gen_suite/rust_tests.rs

use crate::common::init_ai_test_env;
use raise::code_generator::{CodeGeneratorService, TargetLanguage};
use serde_json::json;
use std::fs;

#[test]
fn test_rust_skeleton_generation() {
    let env = init_ai_test_env();

    // CORRECTION : On utilise le dossier temporaire de l'environnement comme sortie
    let service = CodeGeneratorService::new(env._tmp_dir.path().to_path_buf());

    // 1. Donnée Mock (Acteur)
    let actor = json!({
        "id": "uuid-test-pure",
        "name": "Moteur Physique",
        "description": "Simule la gravité.",
        "@type": "oa:OperationalActor"
    });

    // 2. Génération
    let paths = service
        .generate_for_element(&actor, TargetLanguage::Rust)
        .expect("La génération doit réussir");

    // 3. Vérifications
    assert_eq!(paths.len(), 1);
    let file_path = &paths[0];

    assert!(file_path.exists(), "Le fichier généré doit exister");
    assert!(file_path.to_str().unwrap().ends_with("MoteurPhysique.rs"));

    let content = fs::read_to_string(file_path).unwrap();
    assert!(content.contains("pub struct MoteurPhysique"));
    assert!(content.contains("// AI_INJECTION_POINT")); // Vérifie que le hook est là
}
