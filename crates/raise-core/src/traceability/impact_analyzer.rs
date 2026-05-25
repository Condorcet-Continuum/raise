// FICHIER : src-tauri/src/traceability/impact_analyzer.rs

use super::tracer::Tracer;
use crate::utils::prelude::*;

#[derive(Debug, Serializable)]
pub struct ImpactReport {
    pub root_element_id: String,
    pub impacted_elements: Vec<ImpactedItem>,
}

#[derive(Debug, Serializable)]
pub struct ImpactedItem {
    pub element_id: String,
    pub distance: usize,
}

pub struct ImpactAnalyzer {
    tracer: Tracer,
}

impl ImpactAnalyzer {
    pub fn new(tracer: Tracer) -> Self {
        Self { tracer }
    }

    pub fn analyze(&self, element_id: &str, max_depth: usize) -> RaiseResult<ImpactReport> {
        let mut visited = UniqueSet::new();
        let mut impacted = Vec::new();

        if self.tracer.get_downstream_ids(element_id).is_empty()
            && self.tracer.get_upstream_ids(element_id).is_empty()
        {
            raise_error!(
                "ERR_IMPACT_ROOT_NOT_FOUND",
                context = json_value!({"id": element_id})
            );
        }

        self.traverse(element_id, 0, max_depth, &mut visited, &mut impacted)?;

        Ok(ImpactReport {
            root_element_id: element_id.to_string(),
            impacted_elements: impacted,
        })
    }

    fn traverse(
        &self,
        id: &str,
        depth: usize,
        max: usize,
        visited: &mut UniqueSet<String>,
        results: &mut Vec<ImpactedItem>,
    ) -> RaiseResult<()> {
        if depth > max || !visited.insert(id.to_string()) {
            return Ok(());
        }
        if depth > 0 {
            results.push(ImpactedItem {
                element_id: id.to_string(),
                distance: depth,
            });
        }
        for next_id in self.tracer.get_downstream_ids(id) {
            self.traverse(&next_id, depth + 1, max, visited, results)?;
        }
        for next_id in self.tracer.get_upstream_ids(id) {
            self.traverse(&next_id, depth + 1, max, visited, results)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};

    #[test]
    fn test_impact_propagation_pure_graph() -> RaiseResult<()> {
        let mut model = ProjectModel::default();
        let mut p1 = UnorderedMap::new();
        p1.insert("allocatedTo".into(), json_value!("B"));

        // 🎯 FIX : Utilisation de 'add_element' au lieu de model.sa.functions.push
        model.add_element(
            "sa",
            "functions",
            ArcadiaElement {
                id: "A".into(),
                name: NameType::String("A".into()),
                kind: "SystemFunction".into(),
                properties: p1,
            },
        );

        model.add_element(
            "sa",
            "functions",
            ArcadiaElement {
                id: "B".into(),
                name: NameType::String("B".into()),
                kind: "SystemFunction".into(),
                properties: Default::default(),
            },
        );

        let tracer = Tracer::from_legacy_model(&model)?;
        let analyzer = ImpactAnalyzer::new(tracer);

        let report = analyzer.analyze("B", 1)?;

        assert!(
            report.impacted_elements.iter().any(|e| e.element_id == "A"),
            "L'impact n'a pas été propagé de B vers A."
        );

        Ok(())
    }
}
