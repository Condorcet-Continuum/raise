// FICHIER : src-tauri/tests/ai_suite/llm_tests.rs

use crate::common::init_ai_test_env;
use raise::ai::llm::client::LlmBackend;

#[tokio::test]
#[ignore] // Ignoré par défaut (nécessite Docker)
async fn test_local_llm_connectivity() {
    // CORRECTION : init_ai_test_env() est désormais asynchrone.
    // On doit l'attendre pour accéder au membre 'client'.
    let env = init_ai_test_env().await;

    if !env.client.ping_local().await {
        println!("⚠️ SKIPPED: Serveur local introuvable.");
        return;
    }

    let response = env
        .client
        .ask(
            LlmBackend::LocalLlama,
            "Tu es un test unitaire.",
            "Réponds juste 'PONG'.",
        )
        .await;

    assert!(response.is_ok(), "Le LLM local devrait répondre");
    let text = response.unwrap();
    println!("Réponse Locale: {}", text);
    assert!(!text.is_empty());
}

#[tokio::test]
async fn test_cloud_llm_config() {
    // CORRECTION : On ajoute .await ici également pour la cohérence,
    // même si l'environnement n'est pas utilisé directement dans ce test.
    let _env = init_ai_test_env().await;

    let key = std::env::var("RAISE_GEMINI_KEY").unwrap_or_default();

    if key.is_empty() || key.contains("votre_cle") {
        println!("ℹ️ Pas de clé Gemini configurée, vérification ignorée.");
    } else {
        assert!(key.len() > 10, "La clé semble trop courte");
    }
}
