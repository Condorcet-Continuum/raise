// FICHIER : src-tauri/tests/ai_suite/llm_tests.rs

use crate::common::{setup_test_env, LlmMode};
use raise::ai::llm::client::LlmBackend;

#[tokio::test]
#[serial_test::serial] // Protection RTX 5060 en local
#[cfg_attr(not(feature = "cuda"), ignore)]
async fn test_local_llm_connectivity() {
    // CORRECTION : init_ai_test_env() est désormais asynchrone.
    // On doit l'attendre pour accéder au membre 'client'.
    let env = setup_test_env(LlmMode::Enabled).await;

    let response = env
        .client
        .as_ref()
        .expect("LlmClient est requis pour ce test (utilisez LlmMode::Enabled)")
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
