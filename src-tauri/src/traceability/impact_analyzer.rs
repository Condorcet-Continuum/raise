use super::tracer::Tracer;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct ImpactReport {
    pub root_element_id: String,
    pub impacted_elements: Vec<ImpactedItem>,
    pub criticality_score: f32,
}

#[derive(Debug, Serialize)]
pub struct ImpactedItem {
    pub element_id: String,
    pub element_name: String,
    pub distance: usize,
    pub impact_type: String, // "Direct", "Transitive"
}

pub struct ImpactAnalyzer<'a> {
    tracer: Tracer<'a>,
}

impl<'a> ImpactAnalyzer<'a> {
    pub fn new(tracer: Tracer<'a>) -> Self {
        Self { tracer }
    }

    /// Analyse l'impact d'un changement sur un élément donné.
    /// Parcourt le graphe en aval et en amont jusqu'à une certaine profondeur.
    pub fn analyze(&self, element_id: &str, max_depth: usize) -> ImpactReport {
        let mut visited = HashSet::new();
        let mut impacted = Vec::new();

        // Analyse Aval (Downstream) - Ce que cet élément contrôle
        self.traverse(element_id, 0, max_depth, &mut visited, &mut impacted, true);

        // Analyse Amont (Upstream) - Qui dépend de cet élément
        visited.clear(); // On reset pour permettre la bidirectionnalité si besoin
        self.traverse(element_id, 0, max_depth, &mut visited, &mut impacted, false);

        // Dédoublonnage
        impacted.sort_by(|a, b| a.distance.cmp(&b.distance));
        impacted.dedup_by(|a, b| a.element_id == b.element_id);

        let criticality = self.calculate_criticality(&impacted);

        ImpactReport {
            root_element_id: element_id.to_string(),
            impacted_elements: impacted,
            criticality_score: criticality,
        }
    }

    fn traverse(
        &self,
        current_id: &str,
        depth: usize,
        max_depth: usize,
        visited: &mut HashSet<String>,
        results: &mut Vec<ImpactedItem>,
        downstream: bool,
    ) {
        if depth >= max_depth || visited.contains(current_id) {
            return;
        }
        visited.insert(current_id.to_string());

        let neighbors = if downstream {
            self.tracer.get_downstream_elements(current_id)
        } else {
            self.tracer.get_upstream_elements(current_id)
        };

        for neighbor in neighbors {
            results.push(ImpactedItem {
                element_id: neighbor.id.clone(),
                // [CORRECTION] Gestion du type NameType via as_str()
                element_name: neighbor.name.as_str().to_string(),
                distance: depth + 1,
                impact_type: if depth == 0 {
                    "Direct".into()
                } else {
                    "Transitive".into()
                },
            });

            self.traverse(
                &neighbor.id,
                depth + 1,
                max_depth,
                visited,
                results,
                downstream,
            );
        }
    }

    fn calculate_criticality(&self, items: &[ImpactedItem]) -> f32 {
        // Heuristique simple : plus il y a d'éléments touchés, plus c'est critique
        items.len() as f32 * 10.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel}; // [CORRECTION] Import NameType
    use crate::traceability::tracer::Tracer;
    use serde_json::json;
    use std::collections::HashMap;

    // [CORRECTION] Helper de construction robuste
    fn create_element(id: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            // [CORRECTION] Envelopper dans NameType::String
            name: NameType::String(format!("Name {}", id)),
            // [CORRECTION] Champ 'kind' obligatoire
            kind: "Element".to_string(),
            properties,
            // Suppression de ..Default::default()
        }
    }

    #[test]
    fn test_impact_propagation() {
        // Scénario en chaîne : A -> B -> C

        let el_a = create_element("A", json!({ "allocatedTo": ["B"] }));
        let el_b = create_element("B", json!({ "allocatedTo": ["C"] }));
        let el_c = create_element("C", json!({}));

        let mut model = ProjectModel::default();
        model.sa.components = vec![el_a, el_b, el_c];

        let tracer = Tracer::new(&model);
        let analyzer = ImpactAnalyzer::new(tracer);

        let report = analyzer.analyze("B", 5);

        assert_eq!(report.root_element_id, "B");
        assert_eq!(report.impacted_elements.len(), 2);

        // Vérification des IDs impactés
        let ids: Vec<String> = report
            .impacted_elements
            .iter()
            .map(|i| i.element_id.clone())
            .collect();
        assert!(ids.contains(&"A".to_string()));
        assert!(ids.contains(&"C".to_string()));
    }

    #[test]
    fn test_max_depth() {
        // A -> B -> C -> D. Max depth = 1.
        let el_a = create_element("A", json!({ "allocatedTo": ["B"] }));
        let el_b = create_element("B", json!({ "allocatedTo": ["C"] }));
        let el_c = create_element("C", json!({ "allocatedTo": ["D"] }));
        let el_d = create_element("D", json!({}));

        let mut model = ProjectModel::default();
        model.sa.components = vec![el_a, el_b, el_c, el_d];

        let tracer = Tracer::new(&model);
        let analyzer = ImpactAnalyzer::new(tracer);

        let report = analyzer.analyze("A", 1);

        // Doit contenir seulement B (distance 1)
        assert_eq!(report.impacted_elements.len(), 1);
        assert_eq!(report.impacted_elements[0].element_id, "B");
    }
}
