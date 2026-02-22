// FICHIER : src-tauri/src/traceability/compliance/eu_ai_act.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::traceability::tracer::Tracer;
use crate::utils::{prelude::*, HashMap};

pub struct EuAiActChecker;

impl ComplianceChecker for EuAiActChecker {
    fn name(&self) -> &str {
        "EU AI Act (Transparency & Risk Management)"
    }

    /// 🎯 Version robuste : Vérification de la classification des risques et de la transparence
    fn check(&self, _tracer: &Tracer, docs: &HashMap<String, Value>) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        for (id, doc) in docs {
            // Identification sémantique du modèle IA
            let is_ai = doc.get("nature").and_then(|v| v.as_str()) == Some("AI_Model")
                || doc
                    .get("@type")
                    .and_then(|t| t.as_str())
                    .map(|t| t.contains("AI_Model"))
                    .unwrap_or(false);

            if is_ai {
                checked_count += 1;
                let name = doc.get("name").and_then(|n| n.as_str()).unwrap_or(id);

                // 🎯 RÈGLE 1 : Classification du niveau de risque (Obligatoire EU AI Act)
                let risk_level = doc.get("risk_level").and_then(|v| v.as_str());

                if risk_level.is_none() {
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "AI-ACT-RISK-01".to_string(),
                        description: format!(
                            "Le modèle IA '{}' n'a pas de classification de risque (risk_level)",
                            name
                        ),
                        severity: "Critical".to_string(),
                    });
                }

                // 🎯 RÈGLE 2 : Vérification du mode de transparence (Exemple : High Risk nécessite une doc spécifique)
                if risk_level == Some("High")
                    && !doc
                        .get("transparency_certified")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false)
                {
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "AI-ACT-TRANS-01".to_string(),
                        description: format!(
                            "Le modèle à haut risque '{}' manque d'une certification de transparence",
                            name
                        ),
                        severity: "High".to_string(),
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
// TESTS UNITAIRES HYPER ROBUSTES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_eu_ai_act_risk_classification() {
        let mut docs: HashMap<String, Value> = HashMap::new();

        // 1. IA Conforme (Risque défini)
        docs.insert(
            "ai_safe".to_string(),
            json!({
                "id": "ai_safe",
                "nature": "AI_Model",
                "risk_level": "Low"
            }),
        );

        // 2. IA Non Conforme (Risque manquant)
        docs.insert(
            "ai_illegal".to_string(),
            json!({
                "id": "ai_illegal",
                "nature": "AI_Model",
                "name": "BlackBox"
            }),
        );

        // 3. IA Haut Risque sans transparence
        docs.insert(
            "ai_high_risk".to_string(),
            json!({
                "id": "ai_high_risk",
                "nature": "AI_Model",
                "risk_level": "High"
            }),
        );

        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = EuAiActChecker;
        let report = checker.check(&tracer, &docs);

        assert_eq!(report.rules_checked, 3);
        assert_eq!(report.violations.len(), 2); // BlackBox (manque risk) + HighRisk (manque transparence)

        assert!(report
            .violations
            .iter()
            .any(|v| v.element_id == Some("ai_illegal".to_string())));
        assert!(report
            .violations
            .iter()
            .any(|v| v.element_id == Some("ai_high_risk".to_string())));
    }

    #[test]
    fn test_eu_ai_act_ignore_non_ai() {
        let mut docs: HashMap<String, Value> = HashMap::new();
        docs.insert(
            "hardware_v1".to_string(),
            json!({
                "id": "hardware_v1",
                "nature": "Hardware"
            }),
        );

        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = EuAiActChecker;
        let report = checker.check(&tracer, &docs);

        assert!(report.passed);
        assert_eq!(report.rules_checked, 0);
    }
}
