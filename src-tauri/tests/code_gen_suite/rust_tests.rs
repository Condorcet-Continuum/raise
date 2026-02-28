// FICHIER : src-tauri/tests/code_gen_suite/rust_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::code_generator::{CodeGeneratorService, TargetLanguage};
use raise::utils::data::json;
use raise::utils::io;

#[tokio::test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_rust_skeleton_generation() {
    let env = setup_test_env(LlmMode::Enabled).await;

    // 1. Initialisation du service
    let service = CodeGeneratorService::new(env.domain_path.clone());

    // 2. Donnée Mock (Forcer la logique Rust_Crate)
    let actor = json!({
        "id": "uuid-test-pure",
        "name": "Moteur Physique",
        "description": "Simule la gravité.",
        "implementation": {
            "technology": "Rust_Crate",
            "artifactName": "moteur_physique"
        },
        "allocatedFunctions": ["ref:sa:name:Calculer Gravite"]
    });

    // 3. Génération
    let paths = service
        .generate_for_element(&actor, TargetLanguage::Rust)
        .await
        .expect("La génération doit réussir");

    // 4. Vérification Clippy-friendly
    assert!(!paths.is_empty(), "Au moins un fichier doit être généré");

    // On cherche src/lib.rs
    let lib_path = paths
        .iter()
        .find(|p| p.to_string_lossy().contains("src/lib.rs"))
        .expect("Le fichier src/lib.rs est manquant");

    let content = io::read_to_string(lib_path).await.expect("Lecture lib.rs");

    // 5. Assertions sur le fallback de rust_gen.rs
    // Le générateur transforme "Calculer Gravite" en snake_case
    assert!(
        content.contains("pub fn calculer_gravite()"),
        "La fonction Rust est manquante ou mal formatée"
    );
    assert!(
        content.contains("// AI_INJECTION_POINT: calculer_gravite"),
        "Le point d'injection est manquant"
    );
}
