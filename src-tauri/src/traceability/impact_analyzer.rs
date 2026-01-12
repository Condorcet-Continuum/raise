// FICHIER : src-tauri/src/traceability/impact_analyzer.rs

use super::tracer::Tracer;
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct ImpactReport {
    pub root_element_id: String,
    pub impacted_elements: Vec<ImpactedItem>,
}

#[derive(Debug, Serialize)]
pub struct ImpactedItem {
    pub element_id: String,
    pub distance: usize,
}

pub struct ImpactAnalyzer<'a> {
    tracer: Tracer<'a>,
}

impl<'a> ImpactAnalyzer<'a> {
    pub fn new(tracer: Tracer<'a>) -> Self {
        Self { tracer }
    }

    pub fn analyze(&self, element_id: &str, max_depth: usize) -> ImpactReport {
        let mut visited = HashSet::new();
        let mut impacted = Vec::new();
        self.traverse(element_id, 0, max_depth, &mut visited, &mut impacted);

        ImpactReport {
            root_element_id: element_id.to_string(),
            impacted_elements: impacted,
        }
    }

    fn traverse(
        &self,
        id: &str,
        depth: usize,
        max: usize,
        visited: &mut HashSet<String>,
        results: &mut Vec<ImpactedItem>,
    ) {
        if depth > max || !visited.insert(id.to_string()) {
            return;
        }

        if depth > 0 {
            results.push(ImpactedItem {
                element_id: id.to_string(),
                distance: depth,
            });
        }

        for next in self.tracer.get_downstream_elements(id) {
            self.traverse(&next.id, depth + 1, max, visited, results);
        }
        for next in self.tracer.get_upstream_elements(id) {
            self.traverse(&next.id, depth + 1, max, visited, results);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
    use serde_json::json;

    #[test]
    fn test_impact_propagation() {
        let mut model = ProjectModel::default();
        let mut p1 = std::collections::HashMap::new();
        p1.insert("allocatedTo".into(), json!("B"));

        model.sa.functions.push(ArcadiaElement {
            id: "A".into(),
            name: NameType::String("A".into()),
            kind: "F".into(),
            // CORRECTION : Initialisation du champ description
            description: None,
            properties: p1,
        });
        model.sa.functions.push(ArcadiaElement {
            id: "B".into(),
            name: NameType::String("B".into()),
            kind: "F".into(),
            // CORRECTION : Initialisation du champ description
            description: None,
            properties: Default::default(),
        });

        let tracer = Tracer::new(&model);
        let analyzer = ImpactAnalyzer::new(tracer);
        let report = analyzer.analyze("B", 1);

        assert!(report.impacted_elements.iter().any(|e| e.element_id == "A"));
    }
}
