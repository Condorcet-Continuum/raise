use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::ProjectModel;

pub struct Do178cChecker;

impl ComplianceChecker for Do178cChecker {
    fn name(&self) -> &str {
        "DO-178C (Software Considerations in Airborne Systems)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut rules_checked = 0;

        // Règle 1 : Chaque composant logiciel (PA) doit tracer vers une fonction (SA/LA)
        rules_checked += 1;
        for comp in &model.pa.components {
            let has_trace = comp.properties.contains_key("realizedLogicalComponents")
                || comp.properties.contains_key("allocatedFunctions");

            if !has_trace {
                // CORRECTION : utilisation de .as_str() car name est un NameType
                violations.push(Violation {
                    element_id: Some(comp.id.clone()),
                    rule_id: "DO178-HLR-01".into(),
                    description: format!("Le composant logiciel '{}' n'a pas de lien de traçabilité vers les exigences amont.", comp.name.as_str()),
                    severity: "High".into(),
                });
            }
        }

        ComplianceReport {
            standard: self.name().to_string(),
            passed: violations.is_empty(),
            rules_checked,
            violations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel}; // Import de NameType
    use serde_json::json;
    use std::collections::HashMap;

    // Helper adapté à votre types.rs (sans Default)
    fn create_pa_comp(id: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            // CORRECTION : Envelopper la string dans l'Enum NameType
            name: NameType::String(format!("PA Comp {}", id)),
            // CORRECTION : Champ 'kind' obligatoire
            kind: "Component".to_string(),
            properties,
            // On ne peut pas utiliser ..Default::default() ici
        }
    }

    #[test]
    fn test_do178c_traceability_rule() {
        let checker = Do178cChecker;
        let mut model = ProjectModel::default();

        // Cas 1 : Composant PA sans lien (Violation)
        let bad_comp = create_pa_comp("bad_comp", json!({}));

        // Cas 2 : Composant PA avec lien vers Logical Component (Pass)
        let good_comp = create_pa_comp(
            "good_comp",
            json!({
                "realizedLogicalComponents": ["lc_1"]
            }),
        );

        model.pa.components = vec![bad_comp, good_comp];

        let report = checker.check(&model);

        assert_eq!(report.passed, false);
        assert_eq!(report.violations.len(), 1);

        let violation = &report.violations[0];
        assert_eq!(violation.element_id.as_deref(), Some("bad_comp"));
        assert_eq!(violation.rule_id, "DO178-HLR-01");
    }
}
