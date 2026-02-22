// FICHIER : src-tauri/src/traceability/compliance/ai_governance.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::traceability::tracer::Tracer;
use crate::utils::{prelude::*, HashMap}; // 🎯 FIX : Import explicite de HashMap

pub struct AiGovernanceChecker;

impl ComplianceChecker for AiGovernanceChecker {
    fn name(&self) -> &str {
        "RAISE AI Governance"
    }

    fn check(&self, tracer: &Tracer, docs: &HashMap<String, Value>) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        for (id, doc) in docs {
            // Détection du modèle IA
            let is_ai = doc.get("nature").and_then(|v| v.as_str()) == Some("AI_Model");

            if is_ai {
                checked_count += 1;
                let name = doc.get("name").and_then(|n| n.as_str()).unwrap_or(id);

                // 🎯 RECHERCHE DE PREUVES (Reverse Links) via Tracer
                let evidence_ids = tracer.get_upstream_ids(id);

                // FIX : Type annotation explicite pour aider l'inférence de type
                let has_quality = evidence_ids.iter().any(|eid: &String| {
                    docs.get(eid)
                        .map(|d: &Value| {
                            d.get("kind").and_then(|k| k.as_str()) == Some("QualityReport")
                        })
                        .unwrap_or(false)
                });

                let has_xai = evidence_ids.iter().any(|eid: &String| {
                    docs.get(eid)
                        .map(|d: &Value| d.get("kind").and_then(|k| k.as_str()) == Some("XaiFrame"))
                        .unwrap_or(false)
                });

                // 🎯 VALIDATION DES RÈGLES
                if !has_quality {
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "AI-GOV-QR".to_string(),
                        description: format!(
                            "Le modèle IA '{}' manque d'un QualityReport validé",
                            name
                        ),
                        severity: "Critical".to_string(), // 🎯 FIX : .to_string()
                    });
                }

                if !has_xai {
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "AI-GOV-XAI".to_string(),
                        description: format!(
                            "Le modèle IA '{}' manque d'une trame d'explicabilité (XAI)",
                            name
                        ),
                        severity: "High".to_string(), // 🎯 FIX : .to_string()
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
    fn test_audit_ai_model_full_compliance() {
        let mut docs: HashMap<String, Value> = HashMap::new(); // 🎯 FIX : Type explicite

        // Setup : Modèle IA + Ses deux preuves
        docs.insert(
            "AI_1".to_string(),
            json!({ "id": "AI_1", "nature": "AI_Model", "name": "Boreas" }),
        );
        docs.insert(
            "QR_1".to_string(),
            json!({ "id": "QR_1", "kind": "QualityReport", "model_id": "AI_1" }),
        );
        docs.insert(
            "XAI_1".to_string(),
            json!({ "id": "XAI_1", "kind": "XaiFrame", "model_id": "AI_1" }),
        );

        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = AiGovernanceChecker;

        let report = checker.check(&tracer, &docs);

        assert!(report.passed);
        assert_eq!(report.rules_checked, 1);
        assert!(report.violations.is_empty());
    }

    #[test]
    fn test_audit_ai_model_missing_everything() {
        let mut docs: HashMap<String, Value> = HashMap::new();
        // Setup : Modèle IA tout seul
        docs.insert(
            "AI_EMPTY".to_string(),
            json!({ "id": "AI_EMPTY", "nature": "AI_Model" }),
        );

        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = AiGovernanceChecker;

        let report = checker.check(&tracer, &docs);

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 2); // Doit manquer QR et XAI

        let has_qr_violation = report.violations.iter().any(|v| v.rule_id == "AI-GOV-QR");
        let has_xai_violation = report.violations.iter().any(|v| v.rule_id == "AI-GOV-XAI");

        assert!(has_qr_violation);
        assert!(has_xai_violation);
    }
}
