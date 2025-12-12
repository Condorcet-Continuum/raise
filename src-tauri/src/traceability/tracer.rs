use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use std::collections::HashMap;

/// Service principal de traçabilité.
/// Permet de naviguer dans les liens (allocations, réalisations, satisfactions).
pub struct Tracer<'a> {
    model: &'a ProjectModel,
    // Index inversé : Target ID -> List of Source IDs
    reverse_links: HashMap<String, Vec<String>>,
}

impl<'a> Tracer<'a> {
    pub fn new(model: &'a ProjectModel) -> Self {
        let mut tracer = Self {
            model,
            reverse_links: HashMap::new(),
        };
        tracer.index_links();
        tracer
    }

    /// Construit un index inversé pour naviguer "upstream" (vers le haut du V).
    fn index_links(&mut self) {
        // [CORRECTION] Séparation des emprunts pour le Borrow Checker
        // 1. On collecte d'abord toutes les données nécessaires (lecture seule)
        let elements = self.collect_all_elements();

        let mut reverse_map: HashMap<String, Vec<String>> = HashMap::new();

        for element in elements {
            for (key, value) in &element.properties {
                if is_link_property(key) {
                    if let Some(target_id) = value.as_str() {
                        reverse_map
                            .entry(target_id.to_string())
                            .or_default()
                            .push(element.id.clone());
                    } else if let Some(targets) = value.as_array() {
                        for t in targets {
                            if let Some(tid) = t.as_str() {
                                reverse_map
                                    .entry(tid.to_string())
                                    .or_default()
                                    .push(element.id.clone());
                            }
                        }
                    }
                }
            }
        }

        // 2. On met à jour l'état interne (écriture)
        self.reverse_links = reverse_map;
    }

    /// Récupère tous les éléments en amont (qui pointent vers `element_id`).
    pub fn get_upstream_elements(&self, element_id: &str) -> Vec<&ArcadiaElement> {
        if let Some(sources) = self.reverse_links.get(element_id) {
            self.resolve_ids(sources)
        } else {
            Vec::new()
        }
    }

    /// Récupère tous les éléments en aval (pointés par `element_id`).
    pub fn get_downstream_elements(&self, element_id: &str) -> Vec<&ArcadiaElement> {
        let mut results = Vec::new();
        if let Some(element) = self.find_element(element_id) {
            for (key, value) in &element.properties {
                if is_link_property(key) {
                    if let Some(targets) = value.as_array() {
                        for t in targets {
                            if let Some(tid) = t.as_str() {
                                if let Some(el) = self.find_element(tid) {
                                    results.push(el);
                                }
                            }
                        }
                    } else if let Some(tid) = value.as_str() {
                        // [AMELIORATION] Gestion des liens simples (String)
                        if let Some(el) = self.find_element(tid) {
                            results.push(el);
                        }
                    }
                }
            }
        }
        results
    }

    // Helpers
    fn collect_all_elements(&self) -> Vec<&ArcadiaElement> {
        let mut all = Vec::new();
        // [CORRECTION] Utilisation des noms corrects des vecteurs dans les Layers
        all.extend(&self.model.oa.actors);
        all.extend(&self.model.oa.activities); // Supposé exister ou à adapter
        all.extend(&self.model.sa.functions);
        all.extend(&self.model.sa.components);
        all.extend(&self.model.la.components);
        all.extend(&self.model.pa.components);
        all
    }

    fn find_element(&self, id: &str) -> Option<&ArcadiaElement> {
        self.collect_all_elements().into_iter().find(|e| e.id == id)
    }

    fn resolve_ids(&self, ids: &[String]) -> Vec<&ArcadiaElement> {
        ids.iter().filter_map(|id| self.find_element(id)).collect()
    }
}

fn is_link_property(key: &str) -> bool {
    matches!(
        key,
        "allocatedTo"
            | "realizedBy"
            | "realizes"
            | "satisfiedBy"
            | "verifiedBy"
            | "involvedFunctions"
            | "deployedOn"
            | "realizedLogicalComponents" // Ajouté pour DO-178C
            | "allocatedFunctions"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{
        ArcadiaElement,
        NameType, // [CORRECTION] Import NameType
        // [CORRECTION] Import des types corrects pour les Layers si nécessaire
        ProjectModel,
    };
    use serde_json::json;
    use std::collections::HashMap;

    // [CORRECTION] Helper robuste sans Default
    fn create_element(id: &str, name: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(name.to_string()), // [CORRECTION] Enum
            kind: "Element".to_string(),              // Champ obligatoire
            properties,
        }
    }

    // Helper pour créer un modèle vide avec juste les éléments nécessaires
    fn create_mock_model(elements: Vec<ArcadiaElement>) -> ProjectModel {
        let mut model = ProjectModel::default();
        // Pour le test, on injecte tout dans SA (System Analysis) car le Tracer agrège tout
        model.sa.functions = elements;
        model
    }

    #[test]
    fn test_tracer_link_resolution() {
        // Scénario : Func_A --(allocatedTo)--> Comp_B
        let func_a = create_element("func_a", "Function A", json!({ "allocatedTo": ["comp_b"] }));
        let comp_b = create_element("comp_b", "Component B", json!({}));

        let model = create_mock_model(vec![func_a, comp_b]);
        let tracer = Tracer::new(&model);

        // Test Downstream (Aval) : Func_A pointe vers Comp_B
        let downstream = tracer.get_downstream_elements("func_a");
        assert_eq!(downstream.len(), 1);
        assert_eq!(downstream[0].id, "comp_b");

        // Test Upstream (Amont) : Comp_B est pointé par Func_A (Reverse link)
        let upstream = tracer.get_upstream_elements("comp_b");
        assert_eq!(upstream.len(), 1);
        assert_eq!(upstream[0].id, "func_a");
    }

    #[test]
    fn test_single_value_link_property() {
        // Scénario : Req_1 --(satisfiedBy)--> Func_A (Lien simple string, pas array)
        let req_1 = create_element("req_1", "Requirement 1", json!({ "satisfiedBy": "func_a" }));
        let func_a = create_element("func_a", "Function A", json!({}));

        let model = create_mock_model(vec![req_1, func_a]);
        let tracer = Tracer::new(&model);

        let downstream = tracer.get_downstream_elements("req_1");
        assert_eq!(downstream.len(), 1);
        assert_eq!(downstream[0].id, "func_a");
    }
}
