// FICHIER : src-tauri/tests/code_gen_suite/rust_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::code_generator::{CodeGeneratorService, TargetLanguage};
use raise::utils::data::json;
use raise::utils::io;

#[tokio::test]
#[ignore]
async fn test_rust_skeleton_generation() {
    let env = setup_test_env(LlmMode::Enabled).await;

    // On utilise le dossier temporaire de l'environnement comme sortie
    let service = CodeGeneratorService::new(env.domain_path.clone());

    // 1. Donnée Mock (Acteur)
    // Note : On utilise "type" pour matcher la sérialisation interne
    let actor = json!({
        "id": "uuid-test-pure",
        "name": "Moteur Physique",
        "description": "Simule la gravité.",
        // 🎯 Utilise 'kind' avec l'URI complète pour l'alignement Arcadia
        "kind": "https://raise.io/ontology/arcadia/oa#OperationalActor"
    });
    // 2. Génération
    let paths = service
        .generate_for_element(&actor, TargetLanguage::Rust)
        .await
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

    let content = io::read_to_string(file_path)
        .await // ✅ AJOUT DE .await
        .expect("Lecture du fichier généré");

    // Validation du contenu (PascalCase)
    assert!(
        content.contains("pub struct MoteurPhysique"),
        "La structure Rust doit être en PascalCase"
    );

    // Note : L'assertion sur AI_INJECTION_POINT reste commentée car non implémentée
}
