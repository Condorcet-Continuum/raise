use crate::utils::{
    net_client::{get_client, post_json_with_retry},
    prelude::*,
    Duration,
};
#[derive(Clone, Debug)]
pub enum LlmBackend {
    LocalLlama,   // Format OpenAI (llama.cpp)
    GoogleGemini, // Cloud Google
    LlamaCpp,     // Format natif llama-server (Votre Docker 8081)
    RustNative,   // Candle In-Process
}

#[derive(Clone)]
pub struct LlmClient {
    local_url: String,
    gemini_key: String,
    model_name: String,
}

impl LlmClient {
    pub fn new(local_url: &str, gemini_key: &str, model_name: Option<String>) -> Self {
        let raw_model = model_name.unwrap_or_else(|| "gemini-1.5-flash".to_string());

        let clean_model = raw_model
            .trim()
            .trim_matches('"')
            .trim_start_matches("models/")
            .to_string();

        // 1. SANITISATION URL (Localhost -> 127.0.0.1)
        // Force l'IPv4 pour éviter les conflits Docker/Rust sur localhost
        let sanitized_url = local_url
            .trim_end_matches('/')
            .replace("localhost", "127.0.0.1");

        LlmClient {
            local_url: sanitized_url,
            gemini_key: gemini_key.to_string(),
            model_name: clean_model,
        }
    }

    pub async fn ping_local(&self) -> bool {
        let url_health = format!("{}/health", self.local_url);
        let client = get_client(); // Récupère le Singleton depuis utils::net_client

        // On peut toujours surcharger le timeout pour ce ping spécifique
        if client
            .get(&url_health)
            .timeout(Duration::from_secs(1))
            .send()
            .await
            .is_ok()
        {
            return true;
        }

        // Fallback sur /models (Standard OpenAI/llama.cpp)
        let url_models = format!("{}/models", self.local_url);
        match client
            .get(&url_models)
            .timeout(Duration::from_secs(1))
            .send()
            .await
        {
            Ok(res) => res.status().is_success(),
            Err(_) => false,
        }
    }

    // --- LOGIQUE DE RETRY (Anti-Crash Quota) ---
    pub async fn ask(
        &self,
        backend: LlmBackend,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<String> {
        let max_retries = 1;
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self
                .ask_internal(&backend, system_prompt, user_prompt)
                .await
            {
                Ok(response) => return Ok(response),
                Err(err) => {
                    let err_msg = err.to_string();

                    // "Erreur Fatale API" est émise par post_authenticated dans net.rs pour les 400/401
                    if err_msg.contains("404")
                        || err_msg.contains("NOT_FOUND")
                        || err_msg.contains("Connection refused")
                        || err_msg.contains("Erreur Fatale API")
                    {
                        return Err(err);
                    }

                    if attempt >= max_retries {
                        return Err(err);
                    }

                    let wait = Duration::from_secs(2u64.pow(attempt));
                    warn!(
                        "⚠️ Retry ({}/{}). Pause {}s... (Erreur: {})",
                        attempt,
                        max_retries,
                        wait.as_secs(),
                        err_msg
                    );
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }

    async fn ask_internal(&self, backend: &LlmBackend, sys: &str, user: &str) -> Result<String> {
        match backend {
            LlmBackend::LocalLlama => {
                if self.ping_local().await {
                    return self.call_openai_format(&self.local_url, sys, user).await;
                }
                println!("⚠️ Local LLM indisponible, bascule sur Gemini...");
                self.call_google_gemini(sys, user).await
            }
            LlmBackend::LlamaCpp => {
                // Appel direct au backend natif Docker
                self.call_llama_cpp(&self.local_url, sys, user).await
            }
            LlmBackend::GoogleGemini => self.call_google_gemini(sys, user).await,
            LlmBackend::RustNative => Err(AppError::Ai(
                "Le mode RustNative ne doit pas être appelé via HTTP Client.".into(),
            )),
        }
    }

    // --- BACKEND: LLAMA.CPP (Docker Natif) ---
    async fn call_llama_cpp(&self, base_url: &str, sys: &str, user: &str) -> Result<String> {
        let url = format!("{}/completion", base_url);
        let full_prompt = format!("{}\n\n### User:\n{}\n\n### Assistant:\n", sys, user);

        let body = json!({
            "prompt": full_prompt,
            "n_predict": 1024,
            "temperature": 0.7,
            "stop": ["### User:", "User:", "\nUser:", "### Assistant:"]
        });

        // ✅ Magie SSOT : Ta façade gère la requête, le retry, et le parsing JSON !
        let json_resp: Value = post_json_with_retry(&url, &body, 1).await?;

        json_resp["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Ai("Réponse LlamaCpp malformée".into()))
    }

    // --- BACKEND: OPENAI FORMAT (llama.cpp) ---
    async fn call_openai_format(&self, base_url: &str, sys: &str, user: &str) -> Result<String> {
        let url = format!("{}/chat/completions", base_url);
        let body = json!({
            "model": "local-model",
            "messages": [
                { "role": "system", "content": sys },
                { "role": "user", "content": user }
            ],
            "temperature": 0.7
        });

        let json_resp: Value = post_json_with_retry(&url, &body, 1).await?;

        json_resp["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Ai("Réponse locale malformée".into()))
    }
    // --- BACKEND: GOOGLE GEMINI ---

    async fn call_google_gemini(&self, sys: &str, user: &str) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model_name, self.gemini_key
        );

        info!(
            "[TRACE AI] Appel Gemini avec le modèle : '{}'",
            self.model_name
        );

        let combined_prompt = format!("{}\n\nInstruction:\n{}", sys, user);
        let body = json!({
            "contents": [{ "parts": [{ "text": combined_prompt }] }]
        });

        let json_resp: Value = post_json_with_retry(&url, &body, 1).await?;

        json_resp["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::Ai("Structure de réponse inconnue".into()))
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_initialization_and_url_sanitization() {
        // Test 1: Vérification du remplacement de localhost et suppression du slash final
        let client = LlmClient::new("http://localhost:8081/", "dummy_key", None);

        assert_eq!(
            client.local_url, "http://127.0.0.1:8081",
            "L'URL doit être sanitizée (localhost -> 127.0.0.1 et pas de slash final)"
        );
        assert_eq!(
            client.gemini_key, "dummy_key",
            "La clé API doit correspondre"
        );
        assert_eq!(
            client.model_name, "gemini-1.5-flash",
            "Le modèle par défaut doit être attribué"
        );
    }

    #[test]
    fn test_client_model_name_cleaning() {
        // Test 2: Vérification du nettoyage approfondi du nom de modèle
        let client = LlmClient::new(
            "http://127.0.0.1:8080",
            "secret",
            Some("\"models/gemini-2.0-pro\"".to_string()),
        );

        // La logique du constructor doit retirer les guillemets et le préfixe "models/"
        assert_eq!(
            client.model_name, "gemini-2.0-pro",
            "Le nom du modèle doit être nettoyé des guillemets et préfixes inutiles"
        );
    }

    #[tokio::test]
    async fn test_rust_native_backend_rejection() {
        // Test 3: Vérification de la sécurité de routage
        // Le mode RustNative (Candle) ne doit JAMAIS faire de requête HTTP
        let client = LlmClient::new("http://127.0.0.1", "key", None);

        let result = client
            .ask_internal(&LlmBackend::RustNative, "system", "user")
            .await;

        assert!(
            result.is_err(),
            "Le backend RustNative devrait retourner une erreur immédiate"
        );

        if let Err(e) = result {
            let err_msg = e.to_string();
            assert!(
                err_msg.contains("RustNative ne doit pas être appelé via HTTP Client"),
                "Le message d'erreur doit correspondre à la sécurité établie"
            );
        }
    }

    #[tokio::test]
    #[ignore = "Nécessite qu'un LLM local (ex: Llama.cpp) tourne sur le port 8081"]
    async fn test_real_local_ping() {
        // Test 4: Test d'intégration (Ignoré par défaut lors du CI/CD ou de `cargo test`)
        // Tu peux le lancer localement avec : `cargo test test_real_local_ping -- --ignored`

        // Pour que ce test fonctionne, il faut initialiser la config globale (requis par net.rs)
        crate::utils::Once::new().call_once(|| {
            let _ = crate::utils::config::AppConfig::init();
        });

        let client = LlmClient::new("http://127.0.0.1:8081", "dummy", None);

        let is_alive = client.ping_local().await;
        println!("Statut du LLM local : {}", is_alive);
        assert!(is_alive, "Le serveur local devrait être en ligne");
    }
}
