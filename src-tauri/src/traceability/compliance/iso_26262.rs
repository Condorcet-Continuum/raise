use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::ProjectModel;

pub struct Iso26262Checker;

impl ComplianceChecker for Iso26262Checker {
    fn name(&self) -> &str {
        "ISO-26262 (Road vehicles – Functional safety)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut rules_checked = 0;

        // Règle 1 : Propagation ASIL. Si un parent est ASIL-D, l'enfant doit être compatible.
        rules_checked += 1;
        // (Logique simplifiée pour l'exemple)
        for func in &model.sa.functions {
            if let Some(asil) = func.properties.get("asil").and_then(|v| v.as_str()) {
                if asil == "D" && !func.properties.contains_key("safetyMechanism") {
                    violations.push(Violation {
                        element_id: Some(func.id.clone()),
                        rule_id: "ISO26262-ASIL-D".into(),
                        // [CORRECTION] Utilisation de .name.as_str()
                        description: format!("La fonction '{}' est ASIL-D mais ne définit pas de mécanisme de sécurité.", func.name.as_str()),
                        severity: "Critical".into(),
                    });
                }
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
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel}; // [CORRECTION] Import de NameType
    use serde_json::json;
    use std::collections::HashMap;

    // [CORRECTION] Helper de construction manuelle (sans Default)
    fn create_func(id: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            // [CORRECTION] Utilisation de l'Enum NameType
            name: NameType::String(format!("Func {}", id)),
            // [CORRECTION] Ajout du champ kind obligatoire
            kind: "Function".to_string(),
            properties,
        }
    }

    #[test]
    fn test_iso26262_asil_d_check() {
        let checker = Iso26262Checker;
        let mut model = ProjectModel::default();

        // Cas 1 : ASIL-D sans safetyMechanism (Violation)
        let risky_func = create_func("risky", json!({ "asil": "D" }));

        // Cas 2 : ASIL-D avec safetyMechanism (Pass)
        let safe_func = create_func(
            "safe",
            json!({
                "asil": "D",
                "safetyMechanism": "Watchdog"
            }),
        );

        // Cas 3 : ASIL-B sans safetyMechanism (Pass, car règle ne cible que D ici)
        let low_risk_func = create_func("low", json!({ "asil": "B" }));

        model.sa.functions = vec![risky_func, safe_func, low_risk_func];

        let report = checker.check(&model);

        assert_eq!(report.passed, false);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].element_id.as_deref(), Some("risky"));
    }
}
