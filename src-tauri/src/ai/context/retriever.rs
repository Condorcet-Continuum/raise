use crate::ai::nlp::tokenizers;
use crate::model_engine::types::{ArcadiaElement, ProjectModel};

pub struct SimpleRetriever {
    model: ProjectModel,
}

impl SimpleRetriever {
    pub fn new(model: ProjectModel) -> Self {
        Self { model }
    }

    /// Cherche les éléments pertinents dans le modèle basé sur les mots-clés de la requête
    pub fn retrieve_context(&self, query: &str) -> String {
        let query_lower = query.to_lowercase();
        let keywords: Vec<&str> = query_lower.split_whitespace().collect();

        let mut found_elements = Vec::new();

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

        if found_elements.is_empty() {
            return "Aucun élément spécifique du modèle n'a été trouvé.".to_string();
        }

        let mut context_str = String::from("### CONTEXTE DU PROJET (Données réelles) ###\n");

        for (kind, name, description) in found_elements {
            context_str.push_str(&format!("- [{}] {} : {}\n", kind, name, description));
        }

        // Optimisation NLP
        tokenizers::truncate_tokens(&context_str, 2000)
    }

    fn scan_layer(
        &self,
        kind_label: &str,
        elements: &[ArcadiaElement],
        keywords: &[&str],
        results: &mut Vec<(String, String, String)>,
    ) {
        for el in elements {
            let name_lower = el.name.to_lowercase();
            let desc = el
                .properties
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let desc_lower = desc.to_lowercase();

            let matches = keywords
                .iter()
                .any(|&k| k.len() > 3 && (name_lower.contains(k) || desc_lower.contains(k)));

            let ask_all = keywords.contains(&"liste") || keywords.contains(&"tous");

            if matches || ask_all {
                results.push((kind_label.to_string(), el.name.clone(), desc));
            }
        }
    }
}
