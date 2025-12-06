// Simulation d'une fonction d'analyse
fn has_injection_point(code: &str) -> bool {
    // CORRECTION : On retire tous les espaces pour la vérification
    // Cela permet de matcher "//AI_INJECTION_POINT", "// AI...", "//    AI..."
    let clean = code.replace(" ", "");
    clean.contains("//AI_INJECTION_POINT")
}

#[test]
fn test_analyzer_detects_injection_point() {
    let code_with_marker = r#"
        fn execute() {
            // AI_INJECTION_POINT
            println!("Hello");
        }
    "#;

    let code_without_marker = r#"
        fn execute() {
            println!("Just code");
        }
    "#;

    assert!(
        has_injection_point(code_with_marker),
        "Le marqueur doit être détecté"
    );
    assert!(
        !has_injection_point(code_without_marker),
        "Le code standard ne doit pas être flaggé"
    );
}

#[test]
fn test_analyzer_is_robust_to_whitespace() {
    let messy_code = "   //    AI_INJECTION_POINT   ";

    // Ce test va maintenant passer grâce au .replace(" ", "")
    assert!(has_injection_point(messy_code));
}
