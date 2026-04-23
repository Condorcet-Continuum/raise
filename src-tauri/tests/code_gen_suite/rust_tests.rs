// FICHIER : src-tauri/tests/code_gen_suite/rust_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::utils::prelude::*;

// 🎯 FIX : Importation depuis le sous-module 'models' pour la visibilité
use raise::code_generator::models::{CodeElement, CodeElementType, Module, Visibility};
use raise::code_generator::CodeGeneratorService;

#[async_test]
#[serial_test::serial]
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_rust_module_synchronization() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Enabled).await?;
    let domain_root = env.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap();

    // 1. Initialisation du service AST Weaver
    let service = CodeGeneratorService::new(domain_root.clone());

    // 2. Construction sémantique du module (Jumeau Numérique)
    // On simule un module "moteur_physique" contenant une fonction de calcul
    let mut module =
        Module::new("moteur_physique", PathBuf::from("src/lib.rs")).expect("Échec création module");

    let handle = "fn:calculer_gravite";
    module.elements.push(CodeElement {
        // 🎯 NOUVEAUX CHAMPS (Topologie & IA)
        module_id: None,
        parent_id: None,
        attributes: vec![],
        docs: None,
        elements: vec![],

        // Champs existants
        handle: handle.to_string(),
        element_type: CodeElementType::Function,
        visibility: Visibility::Public,
        signature: "fn calculer_gravite()".to_string(),
        body: Some("{ println!(\"9.81 m/s²\"); }".to_string()),
        dependencies: vec![],
        metadata: UnorderedMap::new(),
    });

    // 3. Synchronisation physique (Top-Down)
    let path = service
        .sync_module(module)
        .await
        .expect("La synchronisation AST doit réussir");

    // 4. Vérifications physiques
    assert!(
        path.exists(),
        "Le fichier physique src/lib.rs doit être créé"
    );

    let content = fs::read_to_string_async(&path)
        .await
        .expect("Lecture du code généré");

    // 5. Assertions sur la structure AST Weaver
    // Vérification de la bannière de gouvernance
    assert!(
        content.contains("RAISE GENERATED MODULE : moteur_physique"),
        "Bannière absente"
    );

    // Vérification de l'ancre de réconciliation (Remplace AI_INJECTION_POINT)
    assert!(
        content.contains(&format!("// @raise-handle: {}", handle)),
        "Ancre sémantique de réconciliation manquante"
    );

    // Vérification du code effectif
    assert!(
        content.contains("pub fn calculer_gravite()"),
        "La signature Rust est mal générée ou la visibilité est incorrecte"
    );
    assert!(
        content.contains("9.81 m/s²"),
        "Le corps de la fonction est manquant"
    );

    Ok(())
}
