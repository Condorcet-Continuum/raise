use super::prompts;
use super::response_parser;

// ==========================================
// 1. TESTS UNITAIRES (LOGIQUE INTERNE)
// ==========================================

/// Vérifie que les "Personas" (Prompts Système) sont bien définis et non vides.
#[test]
fn test_prompts_integrity() {
    assert!(
        !prompts::INTENT_CLASSIFIER_PROMPT.trim().is_empty(),
        "Le prompt Intent Classifier est vide !"
    );
    assert!(
        !prompts::SYSTEM_AGENT_PROMPT.trim().is_empty(),
        "Le prompt System Agent est vide !"
    );
    assert!(
        !prompts::SOFTWARE_AGENT_PROMPT.trim().is_empty(),
        "Le prompt Software Agent est vide !"
    );
}

/// Vérifie que le parser nettoie correctement les balises Markdown des LLM.
#[test]
fn test_response_parser_cleaning() {
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

    let json = response_parser::extract_json(raw_markdown)
        .expect("Le parser aurait dû extraire le JSON du Markdown");

    assert_eq!(json["intent"], "CREATE_ELEMENT");
    assert_eq!(json["confidence"], 0.98);

    // Cas 2 : Réponse propre sans Markdown
    let raw_clean = r#"{ "key": "value" }"#;
    let json2 =
        response_parser::extract_json(raw_clean).expect("Le parser aurait dû lire le JSON brut");
    assert_eq!(json2["key"], "value");
}

/// Vérifie que le parser rejette proprement un JSON invalide.
#[test]
fn test_parser_resilience_bad_json() {
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
}
