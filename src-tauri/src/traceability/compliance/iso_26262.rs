// FICHIER : src-tauri/src/traceability/compliance/iso_26262.rs

use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::traceability::tracer::Tracer;
use crate::utils::prelude::*;

pub struct Iso26262Checker;

impl ComplianceChecker for Iso26262Checker {
    fn name(&self) -> &str {
        "ISO-26262 (Road Vehicles Functional Safety)"
    }

    /// 🎯 Version robuste : Vérification des niveaux ASIL pour les composants critiques
    fn check(&self, _tracer: &Tracer, docs: &UnorderedMap<String, JsonValue>) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut checked_count = 0;

        for (id, doc) in docs {
            // 🎯 Détection sémantique : Est-ce un composant critique pour la sécurité ?
            let is_safety_critical = doc
                .get("safety_critical")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            if is_safety_critical {
                checked_count += 1;
                let name = doc.get("name").and_then(|n| n.as_str()).unwrap_or(id);

                // 🎯 RÈGLE : Présence obligatoire du niveau ASIL (Automotive Safety Integrity Level)
                let has_asil = doc.get("asil").is_some();

                if !has_asil {
                    violations.push(Violation {
                        element_id: Some(id.clone()),
                        rule_id: "ISO26262-ASIL-UNDEF".to_string(),
                        description: format!(
                            "Le composant critique '{}' ne possède pas de classification ASIL (A à D)",
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
// TESTS UNITAIRES HYPER ROBUSTES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_iso26262_asil_strict_check() {
        let mut docs: UnorderedMap<String, JsonValue> = UnorderedMap::new();

        // 1. Composant conforme (Critique + ASIL D)
        docs.insert(
            "Brakes_01".to_string(),
            json_value!({
                "_id": "Brakes_01",
                "name": "Electronic Braking System",
                "safety_critical": true,
                "asil": "D"
            }),
        );

        // 2. Composant non conforme (Critique mais ASIL manquant)
        docs.insert(
            "Steering_02".to_string(),
            json_value!({
                "_id": "Steering_02",
                "name": "Power Steering Controller",
                "safety_critical": true
            }),
        );

        // 3. Élément ignoré (Non critique)
        docs.insert(
            "Radio_03".to_string(),
            json_value!({
                "_id": "Radio_03",
                "name": "Infotainment",
                "safety_critical": false
            }),
        );

        // 🎯 Injection du graphe via from_json_list (Isolant total pour le test)
        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = Iso26262Checker;
        let report = checker.check(&tracer, &docs);

        assert_eq!(report.rules_checked, 2); // Brakes + Steering
        assert_eq!(report.violations.len(), 1); // Steering est fautif
        assert_eq!(
            report.violations[0].element_id,
            Some("Steering_02".to_string())
        );
        assert!(report.violations[0].description.contains("ASIL"));
    }

    #[test]
    fn test_iso26262_no_critical_components() {
        let mut docs: UnorderedMap<String, JsonValue> = UnorderedMap::new();
        docs.insert(
            "Lamp".to_string(),
            json_value!({ "_id": "Lamp", "safety_critical": false }),
        );

        let tracer = Tracer::from_json_list(docs.values().cloned().collect());
        let checker = Iso26262Checker;

        let report = checker.check(&tracer, &docs);

        assert!(report.passed);
        assert_eq!(report.rules_checked, 0);
    }
}
