use crate::utils::{data, prelude::*};

/// Tente d'extraire et de parser un objet JSON depuis une réponse brute de LLM.
/// Gère les blocs Markdown (```json ... ```) et le texte superflu.
pub fn extract_json(raw_text: &str) -> RaiseResult<Value> {
    let clean_text = extract_code_block(raw_text);

    // Tentative de parsing
    match data::parse::<Value>(&clean_text) {
        Ok(val) => {
            debug!("JSON extrait avec succès.");
            Ok(val)
        }
        Err(e) => {
            // Si le parsing échoue, on loggue l'erreur et le texte qui a posé problème
            warn!(
                "Échec du parsing JSON. Erreur: {}. Texte reçu: {}",
                e, clean_text
            );
            Err(AppError::from(format!(
                "JSON invalide : {}. Vérifiez la sortie du modèle.",
                e
            )))
        }
    }
}

/// Extrait le contenu situé à l'intérieur des balises de code Markdown.
/// Si aucune balise n'est trouvée, tente de nettoyer le texte pour ne garder que le contenu pertinent (ex: accolades).
pub fn extract_code_block(text: &str) -> String {
    let text = text.trim();

    // 1. Détection des balises Markdown ``` (avec ou sans langage spécifié)
    if let Some(start_fence) = text.find("```") {
        // On cherche la fin de la ligne d'ouverture (ex: ```json\n)
        if let Some(newline_pos) = text[start_fence..].find('\n') {
            let content_start = start_fence + newline_pos + 1;

            // On cherche la balise de fermeture
            if let Some(end_fence) = text[content_start..].rfind("```") {
                return text[content_start..content_start + end_fence]
                    .trim()
                    .to_string();
            }
        }
    }

    // 2. Si pas de Markdown explicite, on utilise une heuristique pour le JSON
    // On cherche la première accolade '{' et la dernière '}'
    if let (Some(first), Some(last)) = (text.find('{'), text.rfind('}')) {
        if first < last {
            return text[first..=last].to_string();
        }
    }

    // 3. Fallback : on renvoie le texte tel quel (nettoyé des espaces)
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_clean_json_from_markdown() {
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

        let result = extract_json(input).unwrap();
        assert_eq!(result["intent"], "CREATE");
        assert_eq!(result["confidence"], 0.9);
    }

    #[test]
    fn test_extract_json_without_markdown() {
        let input = r#"{ "key": "value" }"#;
        let result = extract_json(input).unwrap();
        assert_eq!(result["key"], "value");
    }

    #[test]
    fn test_extract_nested_json() {
        // Cas complexe avec accolades imbriquées sans markdown
        let input = r#"Voici: { "data": { "id": 1 } } merci."#;
        let result = extract_json(input).unwrap();
        assert_eq!(result["data"]["id"], 1);
    }

    #[test]
    fn test_extract_code_block_generic() {
        let input = "```rust\nfn main() {}\n```";
        let code = extract_code_block(input);
        assert_eq!(code, "fn main() {}");
    }
}
