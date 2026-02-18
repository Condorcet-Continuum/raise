// FICHIER : src-tauri/src/ai/context/retriever.rs

use crate::ai::nlp::{preprocessing, tokenizers};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};

pub struct SimpleRetriever {
    model: ProjectModel,
}

impl SimpleRetriever {
    pub fn new(model: ProjectModel) -> Self {
        Self { model }
    }

    /// Récupère un élément "racine" ou par défaut pour servir de contexte initial à la simulation.
    /// Parcourt les couches dans l'ordre pour trouver le premier élément tangible.
    pub fn get_root_element(&self) -> Option<ArcadiaElement> {
        // 1. Operational Analysis (OA)
        if let Some(el) = self.model.oa.actors.first() {
            return Some(el.clone());
        }
        if let Some(el) = self.model.oa.activities.first() {
            return Some(el.clone());
        }

        // 2. System Analysis (SA)
        if let Some(el) = self.model.sa.components.first() {
            return Some(el.clone());
        }
        if let Some(el) = self.model.sa.functions.first() {
            return Some(el.clone());
        }

        // 3. Data
        if let Some(el) = self.model.data.classes.first() {
            return Some(el.clone());
        }

        // 4. AJOUT : Transverse (Si on n'a que des exigences au début du projet)
        if let Some(el) = self.model.transverse.requirements.first() {
            return Some(el.clone());
        }
        if let Some(el) = self.model.transverse.scenarios.first() {
            return Some(el.clone());
        }

        None
    }

    /// Cherche les éléments pertinents avec tolérance aux accents/casse
    pub fn retrieve_context(&self, query: &str) -> String {
        // 1. NORMALISATION DE LA REQUÊTE (via NLP)
        let normalized_query = preprocessing::normalize(query);
        let keywords = tokenizers::tokenize(&normalized_query);

        let mut found_elements = Vec::new();

        // --- SCAN ARCHITECTURE ---
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
            "LA:Composant",
            &self.model.la.components,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "PA:Composant",
            &self.model.pa.components,
            &keywords,
            &mut found_elements,
        );

        // --- SCAN DATA ---
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

        // --- AJOUT : SCAN TRANSVERSE ---
        self.scan_layer(
            "TRANS:Exigence",
            &self.model.transverse.requirements,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "TRANS:Scénario",
            &self.model.transverse.scenarios,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "TRANS:Chaîne",
            &self.model.transverse.functional_chains,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "TRANS:Contrainte",
            &self.model.transverse.constraints,
            &keywords,
            &mut found_elements,
        );
        self.scan_layer(
            "TRANS:Définition",
            &self.model.transverse.common_definitions,
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
        keywords: &[String],
        results: &mut Vec<(String, String, String)>,
    ) {
        for el in elements {
            let raw_name = el.name.as_str();

            // 2. NORMALISATION DES DONNÉES DU MODÈLE
            let name_norm = preprocessing::normalize(raw_name);
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
    use crate::utils::data::HashMap;

    // Helper pour créer un élément factice
    fn mock_el(name: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: "uuid".to_string(),
            name: NameType::String(name.to_string()),
            kind: "test".to_string(),
            description: Some("desc".to_string()),
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_retrieval_normalization() {
        let mut model = ProjectModel::default();
        model.sa.components.push(mock_el("Système Électrique"));

        let retriever = SimpleRetriever::new(model);
        let result = retriever.retrieve_context("Je cherche le systeme electrique");

        assert!(result.contains("Système Électrique"));
    }

    #[test]
    fn test_empty_search() {
        let model = ProjectModel::default();
        let retriever = SimpleRetriever::new(model);
        let result = retriever.retrieve_context("Rien");
        assert!(result.contains("Aucun élément spécifique"));
    }

    #[test]
    fn test_get_root_element() {
        let mut model = ProjectModel::default();
        let retriever_empty = SimpleRetriever::new(model.clone());
        assert!(retriever_empty.get_root_element().is_none());

        model.sa.components.push(mock_el("Composant Racine"));
        let retriever_full = SimpleRetriever::new(model);

        let root = retriever_full.get_root_element();
        assert!(root.is_some());
        assert_eq!(root.unwrap().name.as_str(), "Composant Racine");
    }

    #[test]
    fn test_retrieval_transverse_elements() {
        let mut model = ProjectModel::default();
        // Ajout d'une exigence
        let mut req = mock_el("Perf Constraint 10ms");
        req.description = Some("Le système doit répondre en moins de 10ms".to_string());
        model.transverse.requirements.push(req);

        // Ajout d'un scénario
        model.transverse.scenarios.push(mock_el("Scénario Nominal"));

        let retriever = SimpleRetriever::new(model);

        // 1. Recherche sur Exigence (mot clé "10ms")
        let res_req = retriever.retrieve_context("exigence 10ms");
        assert!(res_req.contains("TRANS:Exigence"), "Label manquant");
        assert!(res_req.contains("Perf Constraint"), "Nom manquant");

        // 2. Recherche sur Scénario
        let res_scen = retriever.retrieve_context("scénario nominal");
        assert!(
            res_scen.contains("TRANS:Scénario"),
            "Label Scénario manquant"
        );
        assert!(
            res_scen.contains("Scénario Nominal"),
            "Nom Scénario manquant"
        );
    }
}
