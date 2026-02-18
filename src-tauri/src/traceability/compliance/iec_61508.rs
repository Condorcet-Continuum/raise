// FICHIER : src-tauri/src/traceability/compliance/iec_61508.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::{NameType, ProjectModel};

pub struct Iec61508Checker;

impl ComplianceChecker for Iec61508Checker {
    fn name(&self) -> &str {
        "IEC-61508 (Industrial Safety)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        // Scan des systèmes complets (souvent en SA ou PA)
        let candidates = [&model.sa.components, &model.pa.components];

        for layer in candidates {
            for comp in layer {
                let is_industrial = comp
                    .properties
                    .get("domain")
                    .and_then(|v| v.as_str())
                    .map(|s| s == "Industrial")
                    .unwrap_or(false);

                if is_industrial {
                    checked_count += 1;
                    let has_sil = comp.properties.contains_key("sil");

                    if !has_sil {
                        let name = match &comp.name {
                            NameType::String(s) => s.clone(),
                            _ => "Inconnu".to_string(),
                        };

                        violations.push(Violation {
                            element_id: Some(comp.id.clone()),
                            rule_id: "IEC61508-SIL-MISSING".to_string(),
                            description: format!(
                                "Système industriel '{}' sans certification SIL",
                                name
                            ),
                            severity: "High".to_string(),
                        });
                    }
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

    fn create_sys(id: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(id.to_string()),
            kind: "System".to_string(),
            // CORRECTION : Initialisation du champ description ajouté récemment
            description: None,
            properties,
        }
    }

    #[test]
    fn test_iec61508_sil_check() {
        let mut model = ProjectModel::default();
        let s1 = create_sys("Turbine", json!({ "domain": "Industrial", "sil": 3 }));
        let s2 = create_sys("Conveyor", json!({ "domain": "Industrial" })); // Manque SIL

        model.pa.components = vec![s1, s2];

        let checker = Iec61508Checker;
        let report = checker.check(&model);

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0].description.contains("Conveyor"));
    }
}
