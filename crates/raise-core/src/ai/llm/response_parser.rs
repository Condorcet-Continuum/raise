use crate::utils::prelude::*;

/// Tente d'extraire et de parser un objet JSON depuis une réponse brute de LLM.
/// Gère les blocs Markdown (```json ... ```) et le texte superflu.
pub fn extract_json(raw_text: &str) -> RaiseResult<JsonValue> {
    let clean_text = extract_code_block(raw_text);

    // Tentative de parsing
    match json::deserialize_from_str::<JsonValue>(&clean_text) {
        Ok(val) => {
            user_debug!(
                "JSON_EXTRACT_SUCCESS",
                json_value!({ "status": "completed" })
            );
            Ok(val)
        }
        Err(e) => {
            // Diagnostic chirurgical pour les sorties de modèles IA
            raise_error!(
                "ERR_JSON_PARSE_FAILED",
                context = json_value!({
                    "action": "parse_extracted_json",
                    "parsing_error": e.to_string(),
                    "raw_text_length": clean_text.len(),
                    "raw_text_preview": clean_text.chars().take(200).collect::<String>(),
                    "hint": "Le modèle a généré un JSON syntaxiquement incorrect. Vérifiez si des balises Markdown (```json) n'ont pas été oubliées ou si le texte est tronqué."
                })
            )
        }
    }
}

/// Extrait le contenu situé à l'intérieur des balises de code Markdown.
/// Si aucune balise n'est trouvée, tente de nettoyer le texte pour ne garder que le contenu pertinent (ex: accolades).
pub fn extract_code_block(text: &str) -> String {
    let text = text.trim();

    // 1. Détection des balises Markdown ``` (avec ou sans langage spécifié)
    if let Some(start_fence) = text.find("```") {
        if let Some(newline_pos) = text[start_fence..].find('\n') {
            let content_start = start_fence + newline_pos + 1;

            if let Some(end_fence) = text[content_start..].rfind("```") {
                return text[content_start..content_start + end_fence]
                    .trim()
                    .to_string();
            }
        }
    }

    // 2. Si pas de Markdown explicite, heuristique pour englober le JSON (Objet ou Tableau)
    let first_brace = text.find('{');
    let last_brace = text.rfind('}');
    let first_bracket = text.find('[');
    let last_bracket = text.rfind(']');

    let start_idx = match (first_brace, first_bracket) {
        (Some(b), Some(k)) => Some(b.min(k)),
        (Some(b), None) => Some(b),
        (None, Some(k)) => Some(k),
        (None, None) => None,
    };

    let end_idx = match (last_brace, last_bracket) {
        (Some(b), Some(k)) => Some(b.max(k)),
        (Some(b), None) => Some(b),
        (None, Some(k)) => Some(k),
        (None, None) => None,
    };

    if let (Some(start), Some(end)) = (start_idx, end_idx) {
        if start < end {
            return text[start..=end].to_string();
        }
    }

    // 3. Fallback : on renvoie le texte tel quel (nettoyé des espaces)
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_clean_json_from_markdown() -> RaiseResult<()> {
        let input = r#"
        Ceci est une réponse.
        ```json
        {
            "intent": "CREATE",
            "confidence": 0.9
        }
        ```
        Fin de la réponse.
        "#;

        let result = extract_json(input)?;
        assert_eq!(result["intent"], "CREATE");
        assert_eq!(result["confidence"], 0.9);
        Ok(())
    }

    #[test]
    fn test_extract_json_without_markdown() -> RaiseResult<()> {
        let input = r#"{ "key": "value" }"#;
        let result = extract_json(input)?;
        assert_eq!(result["key"], "value");
        Ok(())
    }

    #[test]
    fn test_extract_nested_json() -> RaiseResult<()> {
        let input = r#"Voici: { "data": { "id": 1 } } merci."#;
        let result = extract_json(input)?;
        assert_eq!(result["data"]["id"], 1);
        Ok(())
    }

    #[test]
    fn test_extract_json_array() -> RaiseResult<()> {
        let input = r#"Voici les résultats: [{"id": 1}, {"id": 2}] en vrac."#;
        let result = extract_json(input)?;
        assert_eq!(result[0]["id"], 1);
        assert_eq!(result[1]["id"], 2);
        Ok(())
    }

    #[test]
    fn test_extract_code_block_generic() {
        let input = "```rust\nfn main() {}\n```";
        let code = extract_code_block(input);
        assert_eq!(code, "fn main() {}");
    }
}
