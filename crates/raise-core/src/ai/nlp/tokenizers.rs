use crate::ai::nlp::preprocessing;

/// Transforme une phrase brute en vecteur de mots-clés normalisés.
/// Utilisé pour la recherche (Search).
pub fn tokenize(text: &str) -> Vec<String> {
    let normalized = preprocessing::normalize(text);
    let cleaned = preprocessing::remove_stopwords(&normalized);

    cleaned.split_whitespace().map(|s| s.to_string()).collect()
}

/// Tronque une chaîne de caractères pour ne pas dépasser un nombre approximatif de tokens.
/// Utilisé par le Retriever pour limiter la taille du contexte envoyé au LLM.
///
/// NOTE : Ici, on utilise une heuristique simple (1 "mot" = 1 token) pour éviter
/// d'embarquer une dépendance lourde comme HuggingFace Tokenizers.
/// C'est suffisant pour du RAG local.
pub fn truncate_tokens(text: &str, limit: usize) -> String {
    // 1. On découpe en mots (basé sur les espaces)
    let words: Vec<&str> = text.split_whitespace().collect();

    // 2. Si on est en dessous de la limite, on renvoie tout
    if words.len() <= limit {
        return text.to_string();
    }

    // 3. Sinon, on garde les 'limit' premiers mots et on recolle
    // On ajoute "..." pour indiquer la coupure
    let truncated = words[0..limit].join(" ");
    format!("{} ...[tronqué]", truncated)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_simple() {
        let input = "Le chat mange";
        // "le" est stopword -> viré. "chat", "mange" -> gardés.
        assert_eq!(tokenize(input), vec!["chat", "mange"]);
    }

    #[test]
    fn test_truncate_tokens_limit() {
        let text = "Ceci est un test de troncature très long";
        // On garde 3 mots : "Ceci est un"
        let res = truncate_tokens(text, 3);
        assert!(res.contains("Ceci est un"));
        assert!(res.contains("..."));
        assert!(!res.contains("troncature"));
    }

    #[test]
    fn test_truncate_tokens_no_limit() {
        let text = "Court texte";
        let res = truncate_tokens(text, 100);
        assert_eq!(res, "Court texte");
    }
}
