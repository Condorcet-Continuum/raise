use crate::common::init_ai_test_env;
use genaptitude::ai::llm::client::LlmBackend;

#[tokio::test]
#[ignore] // Ignoré par défaut (nécessite Docker)
async fn test_local_llm_connectivity() {
    let env = init_ai_test_env();

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
    let _env = init_ai_test_env();

    let key = std::env::var("GENAPTITUDE_GEMINI_KEY").unwrap_or_default();

    if key.is_empty() || key.contains("votre_cle") {
        println!("ℹ️ Pas de clé Gemini configurée, vérification ignorée.");
    } else {
        assert!(key.len() > 10, "La clé semble trop courte");
    }
}
