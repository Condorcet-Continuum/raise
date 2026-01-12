use reqwest::Client;
use serde_json::{json, Value};
use std::time::Duration;

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
    http_client: Client,
}

impl LlmClient {
    pub fn new(local_url: &str, gemini_key: &str, model_name: Option<String>) -> Self {
        let raw_model = model_name.unwrap_or_else(|| "gemini-1.5-flash-latest".to_string());
        let clean_model = raw_model.replace("models/", "");

        // 1. SANITISATION URL (Localhost -> 127.0.0.1)
        // Force l'IPv4 pour éviter les conflits Docker/Rust sur localhost
        let sanitized_url = local_url
            .trim_end_matches('/')
            .replace("localhost", "127.0.0.1");

        LlmClient {
            local_url: sanitized_url,
            gemini_key: gemini_key.to_string(),
            model_name: clean_model,
            // 2. TIMEOUT AUGMENTÉ (60s -> 180s)
            // Essentiel pour les tests en parallèle ou les machines chargées
            http_client: Client::builder()
                .timeout(Duration::from_secs(180))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn ping_local(&self) -> bool {
        let url_health = format!("{}/health", self.local_url);
        // On teste d'abord /health (Standard Llama.cpp)
        if self
            .http_client
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
        match self
            .http_client
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
    ) -> Result<String, String> {
        let max_retries = 3;
        let mut attempt = 0;

        loop {
            attempt += 1;

            let result = self
                .ask_internal(&backend, system_prompt, user_prompt)
                .await;

            match result {
                Ok(response) => return Ok(response),
                Err(err) => {
                    // Erreurs fatales (Configuration) -> On arrête
                    if err.contains("404")
                        || err.contains("NOT_FOUND")
                        || err.contains("Connection refused")
                    {
                        return Err(err);
                    }

                    // Erreurs temporaires (Quota, Timeout) -> On réessaie
                    if attempt >= max_retries {
                        return Err(err);
                    }

                    // Backoff exponentiel : 2s, 4s, 8s
                    let wait = Duration::from_secs(2u64.pow(attempt));
                    println!(
                        "⚠️ Retry ({}/{}). Pause {}s... (Erreur: {})",
                        attempt,
                        max_retries,
                        wait.as_secs(),
                        err
                    );
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }

    async fn ask_internal(
        &self,
        backend: &LlmBackend,
        sys: &str,
        user: &str,
    ) -> Result<String, String> {
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
            LlmBackend::RustNative => {
                // Ce cas sera géré via une Commande Tauri dédiée, car LlmClient est stateless
                // alors que CandleEngine est stateful (et lourd).
                // Pour l'instant, on bloque l'appel ici.
                Err("Le mode RustNative ne doit pas être appelé via HTTP Client. Utilisez la commande Tauri directe.".to_string())
            }
        }
    }

    // --- BACKEND: LLAMA.CPP (Docker Natif) ---
    async fn call_llama_cpp(
        &self,
        base_url: &str,
        sys: &str,
        user: &str,
    ) -> Result<String, String> {
        let url = format!("{}/completion", base_url);

        // Prompt brut : On injecte le System et le User
        let full_prompt = format!("{}\n\n### User:\n{}\n\n### Assistant:\n", sys, user);

        let body = json!({
            "prompt": full_prompt,
            "n_predict": 1024,
            "temperature": 0.7,
            // 3. STOP TOKENS ROBUSTES
            // On s'assure que le modèle s'arrête s'il tente d'halluciner une suite de conversation
            "stop": ["### User:", "User:", "\nUser:", "### Assistant:"]
        });

        let res = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if !res.status().is_success() {
            return Err(format!("Erreur HTTP LlamaCpp: {}", res.status()));
        }

        let json: Value = res.json().await.map_err(|e| e.to_string())?;

        json["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Réponse LlamaCpp malformée (champ 'content' manquant)".to_string())
    }

    // --- BACKEND: OPENAI FORMAT (llama.cpp) ---
    async fn call_openai_format(
        &self,
        base_url: &str,
        sys: &str,
        user: &str,
    ) -> Result<String, String> {
        let url = format!("{}/chat/completions", base_url);
        let body = json!({
            "model": "local-model",
            "messages": [
                { "role": "system", "content": sys },
                { "role": "user", "content": user }
            ],
            "temperature": 0.7
        });

        let res = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !res.status().is_success() {
            return Err(format!("Erreur HTTP Local: {}", res.status()));
        }
        let json: Value = res.json().await.map_err(|e| e.to_string())?;
        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Réponse locale malformée".to_string())
    }

    // --- BACKEND: GOOGLE GEMINI ---
    async fn call_google_gemini(&self, sys: &str, user: &str) -> Result<String, String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model_name, self.gemini_key
        );

        let combined_prompt = format!("{}\n\nInstruction Utilisateur:\n{}", sys, user);
        let body = json!({ "contents": [{ "parts": [{ "text": combined_prompt }] }] });

        let res = self
            .http_client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;

        let status = res.status();
        let text_response = res.text().await.unwrap_or_default();

        if !status.is_success() {
            return Err(format!("Erreur Gemini Cloud: {}", text_response));
        }

        let json: Value = serde_json::from_str(&text_response).map_err(|e| e.to_string())?;
        if let Some(candidates) = json.get("candidates") {
            if let Some(first) = candidates.get(0) {
                if let Some(content) = first.get("content") {
                    if let Some(parts) = content.get("parts") {
                        if let Some(text) = parts[0].get("text") {
                            return Ok(text.as_str().unwrap_or("").to_string());
                        }
                    }
                }
            }
        }
        Err("Structure JSON Gemini inattendue".to_string())
    }
}
