// FICHIER : src-tauri/src/traceability/compliance/iso_26262.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::{NameType, ProjectModel};

pub struct Iso26262Checker;

impl ComplianceChecker for Iso26262Checker {
    fn name(&self) -> &str {
        "ISO-26262 (Road Vehicles Functional Safety)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        for comp in &model.pa.components {
            // On vérifie seulement si la propriété 'safety_critical' est vraie
            let is_critical = comp
                .properties
                .get("safety_critical")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_critical {
                checked_count += 1;
                let has_asil = comp.properties.contains_key("asil");

                if !has_asil {
                    let name = match &comp.name {
                        NameType::String(s) => s.clone(),
                        _ => "Inconnu".to_string(),
                    };

                    violations.push(Violation {
                        element_id: Some(comp.id.clone()),
                        rule_id: "ISO26262-ASIL-UNDEF".to_string(),
                        description: format!("Composant critique '{}' sans niveau ASIL", name),
                        severity: "Critical".to_string(),
                    });
                }
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

    fn create_comp(id: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(id.to_string()),
            kind: "Component".to_string(),
            // CORRECTION : Initialisation du champ description ajouté récemment
            description: None,
            properties,
        }
    }

    #[test]
    fn test_iso26262_asil_check() {
        let mut model = ProjectModel::default();

        let c1 = create_comp("Brakes", json!({ "safety_critical": true, "asil": "D" }));
        let c2 = create_comp("Radio", json!({ "safety_critical": false }));
        let c3 = create_comp("Airbag_Controller", json!({ "safety_critical": true })); // Pas d'ASIL !

        model.pa.components = vec![c1, c2, c3];

        let checker = Iso26262Checker;
        let report = checker.check(&model);

        assert!(!report.passed);
        assert_eq!(report.rules_checked, 2); // Brakes et Airbag checkés
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0]
            .description
            .contains("Airbag_Controller"));
    }
}
