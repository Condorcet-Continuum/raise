// FICHIER : src-tauri/src/ai/llm/providers/claude.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*; // 🎯 Façade Unique

// =========================================================================
// 1. DTOs (Data Transfer Objects) STRICTEMENT CONFINÉS
// =========================================================================
// Note: On ignore volontairement les champs JSON inutiles ("id", "type", etc.)
// pour minimiser l'empreinte mémoire. Le parseur ignorera les clés absentes.

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct ClaudeMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct ClaudeRequest {
    model: String,
    max_tokens: u32,
    system: String,
    messages: Vec<ClaudeMessage>,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct ClaudeContent {
    text: String,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct ClaudeResponse {
    content: Vec<ClaudeContent>,
}

// =========================================================================
// 2. LOGIQUE PURE (Testable sans réseau)
// =========================================================================

/// Construit le payload spécifique à l'API Messages d'Anthropic
fn build_request(model: &str, system_prompt: &str, user_prompt: &str) -> ClaudeRequest {
    ClaudeRequest {
        model: model.to_string(),
        max_tokens: 4096, // Fixé pour le moment, modifiable via AppConfig si besoin
        system: system_prompt.to_string(),
        messages: vec![ClaudeMessage {
            role: "user".to_string(),
            content: user_prompt.to_string(),
        }],
    }
}

/// Extrait la réponse textuelle du tableau de contenu d'Anthropic
fn extract_text(response: ClaudeResponse) -> RaiseResult<String> {
    let mut content_iter = response.content.into_iter();

    // Zéro Dette : Pattern matching strict
    let first_content = match content_iter.next() {
        Some(c) => c,
        None => raise_error!(
            "ERR_CLAUDE_MALFORMED_RESPONSE",
            error = "L'API a répondu correctement mais n'a retourné aucun contenu.",
            context = json_value!({"action": "extract_content"})
        ),
    };

    Ok(first_content.text)
}

// =========================================================================
// 3. ORCHESTRATION I/O (La fonction appelée par le LlmClient)
// =========================================================================

/// Exécute une requête vers l'API Anthropic Claude via la configuration DB
pub async fn ask(
    manager: &CollectionsManager<'_>,
    system_prompt: &str,
    user_prompt: &str,
) -> RaiseResult<String> {
    // 1. Appel du Gatekeeper (Routage + Vérification d'Activation)
    let settings = match AppConfig::get_runtime_settings(
        manager,
        "ref:services:blueprint:anthropic_claude",
    )
    .await
    {
        Ok(s) => s,
        Err(e) => raise_error!(
            "ERR_CLAUDE_CONFIG_REJECTED",
            error = e.to_string(),
            context = json_value!({"provider": "AnthropicClaude", "hint": "Vérifiez que 'ref:services:blueprint:anthropic_claude' est dans active_services."})
        ),
    };

    // 2. Extraction stricte de la clé API
    let api_key_json = match settings.get("api_key") {
        Some(v) => v,
        None => raise_error!(
            "ERR_CLAUDE_MISSING_API_KEY",
            error = "La clé 'api_key' est absente des réglages du service.",
            context = json_value!({"provider": "AnthropicClaude"})
        ),
    };

    let api_key = match api_key_json.as_str() {
        Some(k) => k,
        None => raise_error!(
            "ERR_CLAUDE_INVALID_API_KEY",
            error = "La clé 'api_key' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "AnthropicClaude"})
        ),
    };

    // 3. Extraction stricte du modèle (SANS FALLBACK)
    let model_json = match settings.get("model") {
        Some(v) => v,
        None => raise_error!(
            "ERR_CLAUDE_MISSING_MODEL",
            error = "La clé 'model' est absente des réglages du service.",
            context = json_value!({"provider": "AnthropicClaude"})
        ),
    };

    let model_name = match model_json.as_str() {
        Some(m) => m,
        None => raise_error!(
            "ERR_CLAUDE_INVALID_MODEL",
            error = "La clé 'model' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "AnthropicClaude"})
        ),
    };

    // 4. Extraction stricte de l'URL
    let url_json = match settings.get("url") {
        Some(u) => u,
        None => raise_error!(
            "ERR_CLAUDE_MISSING_URL",
            error = "La clé 'url' est absente de la configuration du service.",
            context = json_value!({"provider": "AnthropicClaude"})
        ),
    };

    let url = match url_json.as_str() {
        Some(u) => u,
        None => raise_error!(
            "ERR_CLAUDE_INVALID_URL",
            error = "La clé 'url' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "AnthropicClaude"})
        ),
    };

    // 5. Construction pure
    let request_body = build_request(model_name, system_prompt, user_prompt);

    crate::user_info!(
        "NET_LLM_ROUTING",
        json_value!({ "provider": "AnthropicClaude", "model": model_name })
    );

    // 6. Appel HTTP Sécurisé avec Retry & Custom Headers (Zéro Dette Locale)
    let client = get_client();
    let mut attempt = 0;
    let max_retries = 3;
    let mut delay = TimeDuration::from_secs(1);

    let response_data: ClaudeResponse = loop {
        attempt += 1;

        let request_builder = client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01") // Version standard de l'API Messages
            .json(&request_body);

        match request_builder.send().await {
            Ok(response) => {
                let status = response.status();

                if status.is_success() {
                    match response.json::<ClaudeResponse>().await {
                        Ok(data) => break data, // Sortie de boucle victorieuse
                        Err(e) => {
                            raise_error!(
                                "ERR_CLAUDE_JSON_DECODE",
                                error = e.to_string(),
                                context = json_value!({ "attempt": attempt })
                            );
                        }
                    }
                }

                crate::user_warn!(
                    "NET_HTTP_ERROR",
                    json_value!({ "provider": "AnthropicClaude", "status": status.as_u16(), "attempt": attempt })
                );

                // Si c'est une erreur 4xx (autre que Rate Limit), on ne boucle pas : c'est un échec fatal (ex: clé invalide)
                if status.is_client_error() && status != HttpStatusCode::TOO_MANY_REQUESTS {
                    raise_error!(
                        "ERR_CLAUDE_HTTP_FATAL",
                        error = format!("L'API Anthropic a rejeté la requête : HTTP {}", status),
                        context = json_value!({ "url": url, "status": status.as_u16() })
                    );
                }
            }
            Err(e) => {
                crate::user_warn!(
                    "NET_CONN_FAILED",
                    json_value!({ "provider": "AnthropicClaude", "error": e.to_string(), "attempt": attempt })
                );
            }
        }

        if attempt >= max_retries {
            raise_error!(
                "ERR_CLAUDE_MAX_RETRIES",
                error = "Le service Anthropic ne répond pas après plusieurs tentatives.",
                context = json_value!({ "max_retries": max_retries })
            );
        }

        sleep_async(delay).await;
        delay = delay.min(TimeDuration::from_secs(10));
    };

    // 7. Extraction textuelle
    extract_text(response_data)
}

// =========================================================================
// TESTS UNITAIRES (Zéro Dette)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_build_request_formats_correctly() -> RaiseResult<()> {
        let req = build_request("claude-test-model", "Règles système", "Bonjour Claude");

        assert_eq!(req.model, "claude-test-model");
        assert_eq!(req.system, "Règles système");
        assert_eq!(req.max_tokens, 4096);
        assert_eq!(req.messages.len(), 1);
        assert_eq!(req.messages[0].role, "user");
        assert_eq!(req.messages[0].content, "Bonjour Claude");

        Ok(())
    }

    #[test]
    fn test_claude_extract_text_success() -> RaiseResult<()> {
        let mock_response = ClaudeResponse {
            content: vec![ClaudeContent {
                text: "Voici l'analyse demandée.".to_string(),
            }],
        };

        let result = extract_text(mock_response)?;
        assert_eq!(result, "Voici l'analyse demandée.");
        Ok(())
    }

    #[test]
    fn test_claude_extract_text_fails_on_empty_content() -> RaiseResult<()> {
        let mock_response_empty = ClaudeResponse { content: vec![] };

        let result = extract_text(mock_response_empty);

        assert!(
            result.is_err(),
            "L'extracteur aurait dû lever une erreur typée suite à un contenu vide."
        );
        Ok(())
    }
}
