// FICHIER : src-tauri/src/traceability/compliance/eu_ai_act.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::{NameType, ProjectModel};

pub struct EuAiActChecker;

impl ComplianceChecker for EuAiActChecker {
    fn name(&self) -> &str {
        "EU AI Act (Transparency & Record-keeping)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        // On scanne les composants physiques (PA)
        for comp in &model.pa.components {
            // Est-ce un modèle IA ?
            let is_ai = comp
                .properties
                .get("nature")
                .and_then(|v| v.as_str())
                .map(|s| s == "AI_Model")
                .unwrap_or(false);

            if is_ai {
                checked_count += 1;

                // Vérification du niveau de risque
                let has_risk = comp.properties.contains_key("risk_level");

                if !has_risk {
                    let name = match &comp.name {
                        NameType::String(s) => s.clone(),
                        _ => "Inconnu".to_string(),
                    };

                    violations.push(Violation {
                        element_id: Some(comp.id.clone()),
                        rule_id: "AI-ACT-RISK-01".to_string(),
                        description: format!(
                            "Le modèle IA '{}' n'a pas de classification de risque (risk_level)",
                            name
                        ),
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
    use serde_json::json;
    use std::collections::HashMap;

    fn create_ai(id: &str, props: serde_json::Value) -> ArcadiaElement {
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
    fn test_eu_ai_act_risk_missing() {
        let mut model = ProjectModel::default();

        // IA Conforme
        let ai_good = create_ai(
            "ai_good",
            json!({ "nature": "AI_Model", "risk_level": "High" }),
        );
        // IA Non Conforme
        let ai_bad = create_ai("ai_bad", json!({ "nature": "AI_Model" }));
        // Non IA (ignoré)
        let classic = create_ai("classic", json!({ "nature": "Hardware" }));

        model.pa.components = vec![ai_good, ai_bad, classic];

        let checker = EuAiActChecker;
        let report = checker.check(&model);

        assert!(!report.passed);
        assert_eq!(report.rules_checked, 2); // Seulement les 2 IA sont checkées
        assert_eq!(report.violations.len(), 1);
        assert!(report.violations[0].description.contains("ai_bad"));
    }
}
