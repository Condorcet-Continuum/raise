// FICHIER : src-tauri/src/traceability/compliance/do_178c.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::traceability::tracer::Tracer;
use crate::utils::prelude::*;

pub struct Do178cChecker;

impl ComplianceChecker for Do178cChecker {
    fn name(&self) -> &str {
        "DO-178C (Software Traceability)"
    }

    /// 🎯 Version robuste : Audit de la traçabilité SA -> LA
    fn check(&self, tracer: &Tracer, docs: &UnorderedMap<String, JsonValue>) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        for (id, doc) in docs {
            // 🎯 Identification sémantique : Est-ce une fonction système (SA) ?
            let is_system_function = doc
                .get("@type")
                .and_then(|t| t.as_str())
                .map(|t| t.contains("SystemFunction"))
                .unwrap_or(false)
                || doc.get("kind").and_then(|k| k.as_str()) == Some("Function");

            if is_system_function {
                checked_count += 1;

                // 🎯 Vérification de la traçabilité aval (Downstream) en O(1)
                let downstream_ids = tracer.get_downstream_ids(id);

                if downstream_ids.is_empty() {
                    let name = doc.get("name").and_then(|n| n.as_str()).unwrap_or(id);
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "DO178-TRACE-01".to_string(),
                        description: format!(
                            "La fonction '{}' n'est pas allouée à un composant logiciel (LA)",
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

    #[test]
    fn test_do178_traceability_logic() {
        let mut docs: UnorderedMap<String, JsonValue> = UnorderedMap::new();

        // 1. F1 est conforme : allouée à C1 (lien aval)
        docs.insert(
            "F1".to_string(),
            json_value!({
                "_id": "F1",
                "kind": "Function",
                "allocatedTo": "C1"
            }),
        );

        // 2. F2 est en violation : aucune allocation
        docs.insert(
            "F2".to_string(),
            json_value!({
                "_id": "F2",
                "kind": "Function"
            }),
        );

        // 3. Cible du lien
        docs.insert(
            "C1".to_string(),
            json_value!({ "_id": "C1", "kind": "Component" }),
        );

        // 🎯 Injection du graphe via from_json_list (Zéro dépendance ProjectModel)
        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = Do178cChecker;

        let report = checker.check(&tracer, &docs);

        assert_eq!(report.rules_checked, 2);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].element_id, Some("F2".to_string()));
        assert!(report.violations[0].description.contains("F2"));
    }

    #[test]
    fn test_do178_empty_model() {
        let docs = UnorderedMap::new();
        let tracer = Tracer::from_json_list(vec![]);
        let checker = Do178cChecker;

        let report = checker.check(&tracer, &docs);

        assert!(report.passed);
        assert_eq!(report.rules_checked, 0);
    }
}
