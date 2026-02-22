// FICHIER : src-tauri/src/traceability/compliance/mod.rs

pub mod ai_governance;
pub mod do_178c;
pub mod eu_ai_act;
pub mod iec_61508;
pub mod iso_26262;

// Re-exports pour simplifier l'accès
pub use ai_governance::AiGovernanceChecker;
pub use do_178c::Do178cChecker;
pub use eu_ai_act::EuAiActChecker;
pub use iec_61508::Iec61508Checker;
pub use iso_26262::Iso26262Checker;

use crate::traceability::tracer::Tracer;
use crate::utils::{prelude::*, HashMap}; // 🎯 Utilisation de notre façade SSOT

/// Interface universelle de conformité (Audit Engine)
pub trait ComplianceChecker {
    fn name(&self) -> &str;

    /// 🎯 Entrée : Un graphe de liens (Tracer) et un index de documents (ID -> Value)
    /// Ce découplage permet de valider des règles complexes en O(1) sur n'importe quelle donnée.
    fn check(&self, tracer: &Tracer, docs: &HashMap<String, Value>) -> ComplianceReport;
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ComplianceReport {
    pub standard: String,
    pub passed: bool,
    pub rules_checked: usize,
    pub violations: Vec<Violation>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Violation {
    pub element_id: Option<String>,
    pub rule_id: String,
    pub description: String,
    pub severity: String, // "Low", "Medium", "High", "Critical"
}

// =========================================================================
// TESTS UNITAIRES HYPER ROBUSTES (ISOLEMENT TOTAL)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 🎯 TEST 1 : Vérifie que le rapport de conformité survit à l'IPC (Tauri <-> Frontend)
    #[test]
    fn test_robust_serialization_contract() {
        let violation = Violation {
            element_id: Some("id_456".to_string()),
            rule_id: "RULE-X".to_string(),
            description: "Critique".to_string(),
            severity: "High".to_string(),
        };

        let report = ComplianceReport {
            standard: "Standard-Test".to_string(),
            passed: false,
            rules_checked: 10,
            violations: vec![violation],
        };

        let json_str = serde_json::to_string(&report).expect("Serialization failed");
        let recovered: ComplianceReport =
            serde_json::from_str(&json_str).expect("Deserialization failed");

        assert_eq!(report, recovered);
    }

    /// 🎯 TEST 2 : Vérifie que l'interface ComplianceChecker peut naviguer dans un graphe injecté
    struct MockOrphanChecker;
    impl ComplianceChecker for MockOrphanChecker {
        fn name(&self) -> &str {
            "OrphanCheck"
        }
        fn check(&self, tracer: &Tracer, docs: &HashMap<String, Value>) -> ComplianceReport {
            let mut violations = Vec::new();
            // Règle : Chaque élément doit être relié à quelque chose (amont ou aval)
            for id in docs.keys() {
                if tracer.get_downstream_ids(id).is_empty()
                    && tracer.get_upstream_ids(id).is_empty()
                {
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "ORPHAN-01".to_string(),
                        description: "Élément isolé du graphe".to_string(),
                        severity: "Medium".to_string(),
                    });
                }
            }
            ComplianceReport {
                standard: self.name().to_string(),
                passed: violations.is_empty(),
                rules_checked: docs.len(),
                violations,
            }
        }
    }

    #[test]
    fn test_checker_logic_with_injected_graph() {
        let mut docs: HashMap<String, Value> = HashMap::new();
        // A est lié à B. C est seul.
        docs.insert("A".to_string(), json!({ "id": "A", "allocatedTo": "B" }));
        docs.insert("B".to_string(), json!({ "id": "B" }));
        docs.insert("C".to_string(), json!({ "id": "C" }));

        // 🎯 On construit le Tracer en mémoire uniquement pour ce test
        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = MockOrphanChecker;

        let report = checker.check(&tracer, &docs);

        assert_eq!(report.rules_checked, 3);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].element_id, Some("C".to_string()));
    }
}
