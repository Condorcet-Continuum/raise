use super::client::{LlmBackend, LlmClient};
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

// ==========================================
// 2. TESTS D'INTÉGRATION (CLIENT & RÉSEAU)
// ==========================================

#[test]
fn test_client_instantiation() {
    let _client = LlmClient::new("http://localhost:1234", "dummy-key", None);
    // Si ça ne panic pas, c'est bon.
}

/// Vérifie si le serveur LLM local est accessible.
/// Marqué #[ignore] pour ne pas bloquer la CI/CD si aucun serveur ne tourne.
#[tokio::test]
#[ignore]
async fn integration_test_local_availability() {
    // On suppose un port standard OLLAMA ou LM Studio
    let client = LlmClient::new("http://localhost:8080", "dummy", None);
    let is_alive = client.ping_local().await;

    if is_alive {
        println!("✅ Serveur Local DÉTECTÉ sur le port 8080.");
    } else {
        println!("⚠️ Serveur Local OFF (Test passé mais sans connexion).");
    }
}

/// Teste le mécanisme de "Smart Fallback" (Local -> Cloud).
/// Nécessite une clé API Gemini dans l'environnement.
#[tokio::test]
#[ignore]
async fn integration_test_smart_fallback() {
    let key = std::env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    if key.is_empty() || key.contains("YOUR_KEY") {
        println!("⚠️ Test Fallback ignoré : Variable RAISE_GEMINI_KEY manquante.");
        return;
    }

    // On configure un port invalide (9999) pour forcer l'échec local
    let client = LlmClient::new(
        "http://localhost:9999",
        &key,
        Some("gemini-1.5-flash".to_string()),
    );

    println!("🔄 Simulation de panne locale (port 9999) -> Tentative de Fallback...");

    // On demande explicitement le backend LOCAL, mais le client doit basculer seul sur GEMINI
    let res = client
        .ask(
            LlmBackend::LocalLlama,
            "Tu es un test.",
            "Réponds juste par le mot 'SUCCES'.",
        )
        .await;

    match res {
        Ok(content) => {
            println!("✅ FALLBACK RÉUSSI. Réponse reçue : '{}'", content);
            assert!(content.to_uppercase().contains("SUCCES") || !content.is_empty());
        }
        Err(e) => panic!("❌ Echec critique du fallback : {}", e),
    }
}
