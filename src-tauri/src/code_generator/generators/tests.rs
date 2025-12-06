use crate::code_generator::generators::{rust_gen::RustGenerator, LanguageGenerator};
use serde_json::json;

#[test]
fn test_rust_generator_creates_struct_from_actor() {
    // 1. Setup
    let generator = RustGenerator::new();

    // 2. Mock Data (Ce que la DB renverrait)
    let element_doc = json!({
        "name": "Superviseur de Vol",
        "id": "uuid-1234-5678",
        "description": "Gère le traffic aérien.",
        "@type": "oa:OperationalActor"
    });

    // 3. Execution
    let result = generator.generate(&element_doc);

    // 4. Assertions
    assert!(result.is_ok(), "La génération ne doit pas échouer");

    let files = result.unwrap();
    assert_eq!(files.len(), 1, "Doit générer exactement un fichier");

    let file = &files[0];

    // Vérification du nom de fichier (PascalCase + .rs)
    assert_eq!(file.path.to_str().unwrap(), "SuperviseurDeVol.rs");

    // Vérification du contenu
    let code = &file.content;
    assert!(
        code.contains("pub struct SuperviseurDeVol"),
        "Doit contenir la struct"
    );
    assert!(code.contains("uuid-1234-5678"), "Doit contenir l'ID");
    assert!(
        code.contains("// AI_INJECTION_POINT"),
        "Doit contenir le marqueur pour l'IA"
    );
}

#[test]
fn test_rust_generator_handles_missing_fields() {
    let generator = RustGenerator::new();
    // JSON incomplet
    let minimal_doc = json!({
        "name": "Simple"
    });

    let result = generator.generate(&minimal_doc);
    assert!(result.is_ok());

    let file = &result.unwrap()[0];
    assert!(
        file.content.contains("UnknownType"),
        "Doit gérer les types manquants"
    );
    assert!(file.content.contains("0000"), "Doit gérer les ID manquants");
}
