// FICHIER : src-tauri/src/ai/llm/tests.rs

use super::response_parser;
use crate::utils::prelude::*;

// ==========================================
// 1. TESTS UNITAIRES (LOGIQUE INTERNE)
// ==========================================

/// Vérifie que le parser nettoie correctement les balises Markdown des LLM.
#[test]
fn test_response_parser_cleaning() -> RaiseResult<()> {
    // Cas 1 : Réponse "bavarde" avec Markdown
    let raw_markdown = r#"
    Bien sûr, voici le JSON :
    ```json
    {
        "intent": "CREATE_ELEMENT",
        "confidence": 0.98
    }
    ```
    J'espère que cela aide.
    "#;

    let json = match response_parser::extract_json(raw_markdown) {
        Ok(j) => j,
        Err(e) => return Err(e),
    };

    assert_eq!(json["intent"], "CREATE_ELEMENT");
    assert_eq!(json["confidence"], 0.98);

    // Cas 2 : Réponse propre sans Markdown
    let raw_clean = r#"{ "key": "value" }"#;

    // Zéro Dette : Propagation propre au lieu de expect()
    let json2 = response_parser::extract_json(raw_clean)?;
    assert_eq!(json2["key"], "value");

    Ok(())
}

/// Vérifie que le parser rejette proprement un JSON invalide.
#[test]
fn test_parser_resilience_bad_json() -> RaiseResult<()> {
    let bad_response = r#"
    ```json
    {
        "intent": "CHAT",
        // Virgule manquante ou accolade cassée
    "#;

    let result = response_parser::extract_json(bad_response);
    assert!(
        result.is_err(),
        "Le parser doit renvoyer une erreur sur un JSON malformé"
    );
    Ok(())
}
