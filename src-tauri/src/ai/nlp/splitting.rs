use text_splitter::TextSplitter;

/// Découpe un texte long en morceaux (chunks) respectant une limite approximative de caractères.
pub fn split_text_into_chunks(text: &str, max_tokens_per_chunk: usize) -> Vec<String> {
    // Conversion heuristique : 1 token ~= 4 caractères
    let max_chars = max_tokens_per_chunk * 4;

    // CORRECTION v0.28.0 :
    // 1. On passe la taille max directement dans le constructeur new().
    // 2. new(usize) utilise implicitement le compteur de Caractères par défaut.
    let splitter = TextSplitter::new(max_chars);

    // 3. La méthode chunks() ne prend plus que le texte en argument.
    splitter.chunks(text).map(|s| s.to_string()).collect()
}

pub fn split_markdown(text: &str, max_tokens: usize) -> Vec<String> {
    // TextSplitter gère intelligemment la sémantique (paragraphes, etc.) par défaut
    split_text_into_chunks(text, max_tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_splitting() {
        let text = "Premier paragraphe.\n\nDeuxième paragraphe assez long.";
        // On force un découpage très court
        let chunks = split_text_into_chunks(text, 5);

        assert!(!chunks.is_empty());
        println!("Chunks: {:?}", chunks);
    }
}
