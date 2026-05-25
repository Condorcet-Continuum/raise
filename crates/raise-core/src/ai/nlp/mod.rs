// FICHIER : src-tauri/src/ai/nlp/mod.rs

// Modules d'infrastructure NLP
pub mod embeddings;
pub mod preprocessing; // Nettoyage de texte
pub mod splitting; // Découpage en chunks (text-splitter)
pub mod tokenizers; // Découpage en mots // Vectorisation (NativeEngine/BERT)

// Modules d'analyse sémantique (Nouveaux)
pub mod entity_extractor; // Extraction de noms
pub mod parser; // Analyse syntaxique légère

// Tests d'intégration du module NLP
#[cfg(test)]
mod tests {
    use super::tokenizers;

    #[test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_nlp_pipeline_integration() {
        let query = "Je veux l'Architecture du Processeur";
        let keywords = tokenizers::tokenize(query);

        // On vérifie que le pipeline complet fonctionne
        assert!(keywords.contains(&"architecture".to_string()));
        assert!(keywords.contains(&"processeur".to_string()));
        assert!(!keywords.contains(&"je".to_string()));
    }
}
