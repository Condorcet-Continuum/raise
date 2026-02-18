// FICHIER : src-tauri/src/traceability/compliance/do_178c.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::{NameType, ProjectModel};
use crate::traceability::tracer::Tracer;

pub struct Do178cChecker;

impl ComplianceChecker for Do178cChecker {
    fn name(&self) -> &str {
        "DO-178C (Software Considerations in Airborne Systems)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let tracer = Tracer::new(model);
        let mut violations = Vec::new();
        let mut checked_count = 0;

        // Règle 1 : Couverture SA -> LA (Traceability)
        for func in &model.sa.functions {
            checked_count += 1;
            let downstream = tracer.get_downstream_elements(&func.id);

            if downstream.is_empty() {
                let func_name = match &func.name {
                    NameType::String(s) => s.clone(),
                    _ => "Inconnu".to_string(),
                };

                violations.push(Violation {
                    element_id: Some(func.id.clone()),
                    rule_id: "DO178-TRACE-01".to_string(),
                    description: format!(
                        "Fonction système '{}' non allouée (Dead Code potentiel)",
                        func_name
                    ),
                    severity: "High".to_string(),
                });
            }
        }

        ComplianceReport {
            standard: self.name().to_string(),
            passed: violations.is_empty(),
            rules_checked: checked_count,
            violations,
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::utils::{data::json, HashMap};

    fn create_elem(id: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(format!("Elem {}", id)),
            kind: "Function".to_string(),
            // CORRECTION : Initialisation du champ description ajouté récemment
            description: None,
            properties,
        }
    }

    #[test]
    fn test_do178c_traceability() {
        let mut model = ProjectModel::default();

        // F1 est couverte (allouée à C1)
        let f1 = create_elem("f1", json!({ "allocatedTo": ["c1"] }));
        // F2 est orpheline
        let f2 = create_elem("f2", json!({}));

        // Cible (pour que le Tracer fonctionne)
        let c1 = create_elem("c1", json!({}));

        model.sa.functions = vec![f1, f2];
        model.la.components = vec![c1];

        let checker = Do178cChecker;
        let report = checker.check(&model);

        assert!(!report.passed);
        assert_eq!(report.rules_checked, 2);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].element_id, Some("f2".to_string()));
    }
}
