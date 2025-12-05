use tiktoken_rs::cl100k_base;

/// Estime le nombre de tokens dans une chaîne de caractères.
/// Utilise l'encodage `cl100k_base` (standard GPT-4/Mistral).
pub fn count_tokens(text: &str) -> usize {
    // En cas d'erreur d'init du tokenizer (rare), on fallback sur une heuristique
    // Heuristique : ~4 caractères par token en moyenne pour l'anglais/code, un peu moins en FR.
    match cl100k_base() {
        Ok(bpe) => bpe.encode_with_special_tokens(text).len(),
        Err(_) => text.len() / 4,
    }
}

/// Tronque un texte pour qu'il ne dépasse pas un nombre max de tokens.
/// Utile pour limiter l'historique de conversation.
pub fn truncate_tokens(text: &str, max_tokens: usize) -> String {
    let bpe = match cl100k_base() {
        Ok(b) => b,
        Err(_) => return text.chars().take(max_tokens * 4).collect(),
    };

    let tokens = bpe.encode_with_special_tokens(text);
    if tokens.len() <= max_tokens {
        return text.to_string();
    }

    // On décode uniquement les N premiers tokens
    let kept_tokens = &tokens[..max_tokens];
    bpe.decode(kept_tokens.to_vec())
        .unwrap_or_else(|_| text.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_count() {
        let text = "Bonjour GenAptitude";
        let count = count_tokens(text);
        // "Bonjour" + " Gen" + "Apt" + "itude" ou similaire
        assert!(count > 0 && count < 10);
    }

    #[test]
    fn test_truncate() {
        let text = "Ceci est un texte très long qui doit être coupé.";
        let truncated = truncate_tokens(text, 2);
        // Devrait garder environ 2 mots/bouts
        assert!(count_tokens(&truncated) <= 2);
    }
}
