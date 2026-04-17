// FICHIER : src-tauri/src/ai/nlp/preprocessing.rs
use crate::utils::prelude::*;

/// Normalise le texte :
/// 1. Minuscule & Sans accents.
/// 2. Remplace la ponctuation par des espaces (CORRECTIF CRITIQUE : l'arc -> l arc).
/// 3. Retire les espaces multiples.
pub fn normalize(text: &str) -> String {
    let text = text.trim().to_lowercase();
    let text_no_accents = remove_accents(&text);

    // REMPLACEMENT : On mappe les caractères non-alphanumériques vers des espaces
    // au lieu de les supprimer purement et simplement.
    let with_spaces: String = text_no_accents
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    // On nettoie les espaces multiples créés par le remplacement
    with_spaces
        .split_whitespace()
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Gestion manuelle des accents.
fn remove_accents(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'à' | 'â' | 'ä' => 'a',
            'ç' => 'c',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ÿ' => 'y',
            _ => c,
        })
        .collect()
}

/// Supprime les mots vides (Stop Words) français.
pub fn remove_stopwords(text: &str) -> String {
    let stopwords = get_french_stopwords();
    text.split_whitespace()
        .filter(|word| !stopwords.contains(*word))
        .collect::<Vec<&str>>()
        .join(" ")
}

/// Dictionnaire statique chargé une seule fois en mémoire (O(1) allocation)
fn get_french_stopwords() -> &'static UniqueSet<&'static str> {
    static STOPWORDS: StaticCell<UniqueSet<&'static str>> = StaticCell::new();

    STOPWORDS.get_or_init(|| {
        let mut set = UniqueSet::new();
        let list = [
            "le", "la", "les", "l", "un", "une", "des", "du", "de", "d", "ce", "cet", "cette",
            "ces", "mon", "ton", "son", "et", "ou", "mais", "donc", "car", "ni", "à", "en", "dans",
            "par", "pour", "sur", "avec", "sans", "qui", "que", "quoi", "dont", "où", "est",
            "sont", "avoir", "être", "je", "tu", "il", "nous", "vous", "veut", "voudrais",
        ];
        for word in list {
            set.insert(word);
        }
        set
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_accent() {
        assert_eq!(normalize("Hélène"), "helene");
    }

    #[test]
    fn test_normalize_punctuation() {
        // Test du cas qui a fait échouer votre pipeline
        assert_eq!(normalize("l'architecture"), "l architecture");
    }
}
