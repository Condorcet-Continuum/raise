// FICHIER : src-tauri/src/ai/llm/providers/mistral.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*; // 🎯 Façade Unique

// =========================================================================
// 1. DTOs (Data Transfer Objects) STRICTEMENT CONFINÉS
// =========================================================================

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct MistralMessage {
    role: String,
    content: String,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct MistralRequest {
    model: String,
    messages: Vec<MistralMessage>,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct MistralChoice {
    message: MistralMessage,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct MistralResponse {
    choices: Vec<MistralChoice>,
}

// =========================================================================
// 2. LOGIQUE PURE (Testable sans réseau)
// =========================================================================

/// Formate les messages pour Mistral (Gère nativement les rôles system/user)
fn build_request(model: &str, system_prompt: &str, user_prompt: &str) -> MistralRequest {
    MistralRequest {
        model: model.to_string(),
        messages: vec![
            MistralMessage {
                role: "system".to_string(),
                content: system_prompt.to_string(),
            },
            MistralMessage {
                role: "user".to_string(),
                content: user_prompt.to_string(),
            },
        ],
    }
}

/// Extrait la réponse textuelle du tableau de choix de Mistral
fn extract_text(response: MistralResponse) -> RaiseResult<String> {
    let mut choices_iter = response.choices.into_iter();

    // Zéro Dette : Match strict
    let first_choice = match choices_iter.next() {
        Some(c) => c,
        None => raise_error!(
            "ERR_MISTRAL_MALFORMED_RESPONSE",
            error = "L'API a répondu correctement mais n'a retourné aucun choix.",
            context = json_value!({"action": "extract_choices"})
        ),
    };

    Ok(first_choice.message.content)
}

// =========================================================================
// 3. ORCHESTRATION I/O (La fonction appelée par le LlmClient)
// =========================================================================

/// Exécute une requête vers l'API Mistral AI via la configuration DB
pub async fn ask(
    manager: &CollectionsManager<'_>,
    system_prompt: &str,
    user_prompt: &str,
) -> RaiseResult<String> {
    // 1. Appel du Gatekeeper (Routage + Vérification d'Activation)
    let settings = match AppConfig::get_runtime_settings(
        manager,
        "ref:services:blueprint:mistral_ai",
    )
    .await
    {
        Ok(s) => s,
        Err(e) => raise_error!(
            "ERR_MISTRAL_CONFIG_REJECTED",
            error = e.to_string(),
            context = json_value!({"provider": "MistralAI", "hint": "Vérifiez que 'ref:services:blueprint:mistral_ai' est dans active_services."})
        ),
    };

    let api_key = match settings.get("api_key").and_then(|v| v.as_str()) {
        Some(k) => k,
        None => raise_error!(
            "ERR_MISTRAL_MISSING_API_KEY",
            error = "La clé 'api_key' est absente ou n'est pas une chaîne valide.",
            context = json_value!({"provider": "MistralAI"})
        ),
    };

    // 3. Extraction stricte du modèle (SANS FALLBACK)
    let model_json = match settings.get("model") {
        Some(v) => v,
        None => raise_error!(
            "ERR_MISTRAL_MISSING_MODEL",
            error = "La clé 'model' est absente des réglages du service.",
            context = json_value!({"provider": "MistralAI"})
        ),
    };

    let model_name = match model_json.as_str() {
        Some(m) => m,
        None => raise_error!(
            "ERR_MISTRAL_INVALID_MODEL",
            error = "La clé 'model' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "MistralAI"})
        ),
    };

    // 4. Extraction stricte de l'URL
    let url_json = match settings.get("url") {
        Some(u) => u,
        None => raise_error!(
            "ERR_MISTRAL_MISSING_URL",
            error = "La clé 'url' est absente de la configuration du service.",
            context = json_value!({"provider": "MistralAI"})
        ),
    };

    let url = match url_json.as_str() {
        Some(u) => u,
        None => raise_error!(
            "ERR_MISTRAL_INVALID_URL",
            error = "La clé 'url' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "MistralAI"})
        ),
    };

    // 5. Construction pure
    let request_body = build_request(model_name, system_prompt, user_prompt);

    crate::user_info!(
        "NET_LLM_ROUTING",
        json_value!({ "provider": "MistralAI", "model": model_name })
    );

    // 6. Appel HTTP asynchrone utilisant la Façade (Bearer Auth + Retry automatique)
    let response: MistralResponse = post_authenticated_async(
        url,
        &request_body,
        Some(api_key),
        3, // max_retries
    )
    .await?;

    // 7. Extraction textuelle
    extract_text(response)
}

// =========================================================================
// TESTS UNITAIRES (Zéro Dette)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mistral_build_request_formats_correctly() -> RaiseResult<()> {
        let req = build_request(
            "mistral-large-latest",
            "Tu es un expert en Rust.",
            "Code une boucle.",
        );

        assert_eq!(req.model, "mistral-large-latest");
        assert_eq!(req.messages.len(), 2);

        assert_eq!(req.messages[0].role, "system");
        assert_eq!(req.messages[0].content, "Tu es un expert en Rust.");

        assert_eq!(req.messages[1].role, "user");
        assert_eq!(req.messages[1].content, "Code une boucle.");

        Ok(())
    }

    #[test]
    fn test_mistral_extract_text_success() -> RaiseResult<()> {
        let mock_response = MistralResponse {
            choices: vec![MistralChoice {
                message: MistralMessage {
                    role: "assistant".to_string(),
                    content: "Voici le code : loop {}".to_string(),
                },
            }],
        };

        let result = extract_text(mock_response)?;
        assert_eq!(result, "Voici le code : loop {}");
        Ok(())
    }

    #[test]
    fn test_mistral_extract_text_fails_on_empty_choices() -> RaiseResult<()> {
        let mock_response_empty = MistralResponse { choices: vec![] };

        let result = extract_text(mock_response_empty);

        assert!(
            result.is_err(),
            "L'extracteur aurait dû lever une erreur typée suite à un tableau vide."
        );
        Ok(())
    }
}
