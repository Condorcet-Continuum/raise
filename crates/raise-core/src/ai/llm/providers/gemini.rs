// FICHIER : src-tauri/src/ai/llm/providers/gemini.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::data::config::AppConfig;
use crate::utils::prelude::*;

// =========================================================================
// 1. DTOs (Data Transfer Objects) STRICTEMENT CONFINÉS
// =========================================================================

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct GeminiPart {
    text: String,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct GeminiContent {
    parts: Vec<GeminiPart>,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct GeminiRequest {
    contents: Vec<GeminiContent>,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct GeminiCandidate {
    content: GeminiContent,
}

#[derive(Debug, Serializable, Deserializable, Clone, PartialEq)]
struct GeminiResponse {
    candidates: Vec<GeminiCandidate>,
}

// =========================================================================
// 2. LOGIQUE PURE (Testable sans réseau)
// =========================================================================

/// Construit l'URL finale à partir du patron issu de la base de données
fn build_url(template: &str, model: &str, api_key: &str) -> String {
    template.replacen("{}", model, 1).replacen("{}", api_key, 1)
}

/// Formate les directives système et le prompt utilisateur pour Gemini
fn build_request(system_prompt: &str, user_prompt: &str) -> GeminiRequest {
    let combined_prompt = format!(
        "System Rules:\n{}\n\nUser Request:\n{}",
        system_prompt, user_prompt
    );

    GeminiRequest {
        contents: vec![GeminiContent {
            parts: vec![GeminiPart {
                text: combined_prompt,
            }],
        }],
    }
}

/// Extrait la réponse textuelle de la structure complexe de Google
fn extract_text(response: GeminiResponse) -> RaiseResult<String> {
    let mut candidates_iter = response.candidates.into_iter();

    // Zéro Dette : Match strict au lieu de and_then()
    let mut first_candidate = match candidates_iter.next() {
        Some(c) => c,
        None => raise_error!(
            "ERR_GEMINI_MALFORMED_RESPONSE",
            error = "L'API a répondu correctement mais n'a retourné aucun candidat.",
            context = json_value!({"action": "extract_candidates"})
        ),
    };

    // Zéro Dette : Match strict au lieu de ok_or_else()
    let first_part = match first_candidate.content.parts.pop() {
        Some(p) => p,
        None => raise_error!(
            "ERR_GEMINI_MALFORMED_RESPONSE",
            error = "Le candidat retourné par l'API ne contient aucun texte exploitable.",
            context = json_value!({"action": "extract_parts"})
        ),
    };

    Ok(first_part.text)
}

// =========================================================================
// 3. ORCHESTRATION I/O (La fonction appelée par le LlmClient)
// =========================================================================

/// Exécute une requête vers l'API Google Gemini en utilisant la configuration DB
pub async fn ask(
    manager: &CollectionsManager<'_>,
    system_prompt: &str,
    user_prompt: &str,
) -> RaiseResult<String> {
    // 1. Appel du Gatekeeper (Routage + Vérification d'Activation)
    let settings = match AppConfig::get_runtime_settings(
        manager,
        "ref:services:blueprint:google_gemini",
    )
    .await
    {
        Ok(s) => s,
        Err(e) => raise_error!(
            "ERR_GEMINI_CONFIG_REJECTED",
            error = e.to_string(),
            context = json_value!({"provider": "GoogleGemini", "hint": "Vérifiez que 'ref:services:blueprint:google_gemini' est dans active_services."})
        ),
    };

    // 2. Extraction stricte de la clé API
    let api_key_json = match settings.get("api_key") {
        Some(v) => v,
        None => raise_error!(
            "ERR_GEMINI_MISSING_API_KEY",
            error = "La clé 'api_key' est absente des réglages du service.",
            context = json_value!({"provider": "GoogleGemini"})
        ),
    };

    let api_key = match api_key_json.as_str() {
        Some(k) => k,
        None => raise_error!(
            "ERR_GEMINI_INVALID_API_KEY",
            error = "La clé 'api_key' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "GoogleGemini"})
        ),
    };

    // 3. Extraction stricte du modèle (avec Fallback propre)
    let model_json = match settings.get("model") {
        Some(v) => v,
        None => raise_error!(
            "ERR_GEMINI_MISSING_MODEL",
            error = "La clé 'model' est absente des réglages du service.",
            context = json_value!({"provider": "GoogleGemini"})
        ),
    };
    let model_name = match model_json.as_str() {
        Some(m) => m,
        None => raise_error!(
            "ERR_GEMINI_INVALID_MODEL",
            error = "La clé 'model' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "GoogleGemini"})
        ),
    };

    // 4. Extraction stricte de l'URL
    let url_template_json = match settings.get("url") {
        Some(u) => u,
        None => raise_error!(
            "ERR_GEMINI_MISSING_URL",
            error = "Le patron 'url' est absent de la configuration du service.",
            context = json_value!({"provider": "GoogleGemini"})
        ),
    };

    let url_template = match url_template_json.as_str() {
        Some(u) => u,
        None => raise_error!(
            "ERR_GEMINI_INVALID_URL",
            error = "Le patron 'url' n'est pas une chaîne de caractères valide.",
            context = json_value!({"provider": "GoogleGemini"})
        ),
    };

    // 5. Construction pure
    let url = build_url(url_template, model_name, api_key);
    let request_body = build_request(system_prompt, user_prompt);

    crate::user_info!(
        "NET_LLM_ROUTING",
        json_value!({ "provider": "GoogleGemini", "model": model_name })
    );

    // 6. Appel asynchrone via la façade
    let response: GeminiResponse = post_json_with_retry_async(&url, &request_body, 3).await?;

    // 7. Extraction du texte final
    extract_text(response)
}

// =========================================================================
// TESTS UNITAIRES (Zéro Dette)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_url_replaces_correctly() -> RaiseResult<()> {
        let template = "https://api.google.com/models/{}:generateContent?key={}";
        let model = "gemini-1.5-flash";
        let key = "ABC_123";

        let url = build_url(template, model, key);

        assert_eq!(
            url,
            "https://api.google.com/models/gemini-1.5-flash:generateContent?key=ABC_123"
        );
        Ok(())
    }

    #[test]
    fn test_build_request_formats_prompts() -> RaiseResult<()> {
        let sys = "Tu es un assistant.";
        let usr = "Combien font 2+2 ?";

        let request = build_request(sys, usr);
        let expected_text =
            "System Rules:\nTu es un assistant.\n\nUser Request:\nCombien font 2+2 ?";

        assert_eq!(request.contents.len(), 1);
        assert_eq!(request.contents[0].parts.len(), 1);
        assert_eq!(request.contents[0].parts[0].text, expected_text);

        Ok(())
    }

    #[test]
    fn test_extract_text_success() -> RaiseResult<()> {
        let mock_response = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    parts: vec![GeminiPart {
                        text: "Voici la réponse générée.".to_string(),
                    }],
                },
            }],
        };

        let result = extract_text(mock_response)?;
        assert_eq!(result, "Voici la réponse générée.");
        Ok(())
    }

    #[test]
    fn test_extract_text_fails_on_empty_candidates() -> RaiseResult<()> {
        let mock_response_empty = GeminiResponse { candidates: vec![] };

        let result = extract_text(mock_response_empty);
        assert!(
            result.is_err(),
            "L'extracteur aurait dû lever une erreur typée (aucun candidat)."
        );
        Ok(())
    }

    #[test]
    fn test_extract_text_fails_on_empty_parts() -> RaiseResult<()> {
        let mock_response_empty_parts = GeminiResponse {
            candidates: vec![GeminiCandidate {
                content: GeminiContent {
                    parts: vec![], // Tableau vide
                },
            }],
        };

        let result = extract_text(mock_response_empty_parts);
        assert!(
            result.is_err(),
            "L'extracteur aurait dû lever une erreur typée (aucune partie de texte)."
        );
        Ok(())
    }
}
