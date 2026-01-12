// FICHIER : src-tauri/src/ai/context/retriever.rs

// AJOUT : Import du preprocessing pour la normalisation
use crate::ai::nlp::{preprocessing, tokenizers};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};

pub struct SimpleRetriever {
    model: ProjectModel,
}

impl SimpleRetriever {
    pub fn new(model: ProjectModel) -> Self {
        Self { model }
    }

    /// Cherche les éléments pertinents avec tolérance aux accents/casse
    pub fn retrieve_context(&self, query: &str) -> String {
        // 1. NORMALISATION DE LA REQUÊTE (via NLP)
        // "Système" -> "systeme"
        let normalized_query = preprocessing::normalize(query);
        let keywords = tokenizers::tokenize(&normalized_query);

        let mut found_elements = Vec::new();

        // Scan des couches (inchangé mais utilise la nouvelle logique de scan)
        self.scan_layer(
            "OA:Acteur",
            &self.model.oa.actors,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "OA:Activité",
            &self.model.oa.activities,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "SA:Fonction",
            &self.model.sa.functions,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "SA:Composant",
            &self.model.sa.components,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "DATA:Class",
            &self.model.data.classes,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "DATA:Item",
            &self.model.data.exchange_items,
            &keywords,
            &mut found_elements,
        );

        if found_elements.is_empty() {
            return "Aucun élément spécifique du modèle n'a été trouvé.".to_string();
        }

        let mut context_str = String::from("### CONTEXTE DU PROJET (Données réelles) ###\n");
        for (kind, name, description) in found_elements {
            context_str.push_str(&format!("- [{}] {} : {}\n", kind, name, description));
        }

        tokenizers::truncate_tokens(&context_str, 2000)
    }

    fn scan_layer(
        &self,
        kind_label: &str,
        elements: &[ArcadiaElement],
        keywords: &[String], // Changement: Vec<String> car tokenize renvoie des String
        results: &mut Vec<(String, String, String)>,
    ) {
        for el in elements {
            let raw_name = el.name.as_str();

            // 2. NORMALISATION DES DONNÉES DU MODÈLE
            // On normalise le nom et la description pour la comparaison
            let name_norm = preprocessing::normalize(raw_name);

            // CORRECTION: On utilise le champ `description` dédié
            let raw_desc = el.description.as_deref().unwrap_or("");
            let desc_norm = preprocessing::normalize(raw_desc);

            // 3. MATCHING ROBUSTE
            let matches = keywords
                .iter()
                .any(|k| k.len() > 3 && (name_norm.contains(k) || desc_norm.contains(k)));

            let ask_all = keywords.iter().any(|k| k == "liste" || k == "tous");

            if matches || ask_all {
                results.push((
                    kind_label.to_string(),
                    raw_name.to_string(),
                    raw_desc.to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use std::collections::HashMap;

    // Helper pour créer un élément factice
    fn mock_el(name: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: "uuid".to_string(),
            name: NameType::String(name.to_string()),
            kind: "test".to_string(),
            // CORRECTION : Utilisation du champ description
            description: Some("desc".to_string()),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_retrieval_normalization() {
        let mut model = ProjectModel::default();
        // On ajoute "Système Électrique" avec accents et majuscules
        model.sa.components.push(mock_el("Système Électrique"));

        let retriever = SimpleRetriever::new(model);

        // On cherche "systeme electrique" (minuscule sans accent)
        let result = retriever.retrieve_context("Je cherche le systeme electrique");

        // Ça doit matcher grâce au preprocessing
        assert!(result.contains("Système Électrique"));
    }

    #[test]
    fn test_empty_search() {
        let model = ProjectModel::default();
        let retriever = SimpleRetriever::new(model);
        let result = retriever.retrieve_context("Rien");
        assert!(result.contains("Aucun élément spécifique"));
    }
}
