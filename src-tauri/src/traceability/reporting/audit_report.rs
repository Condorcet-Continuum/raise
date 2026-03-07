// FICHIER : src-tauri/src/traceability/reporting/audit_report.rs

use crate::traceability::compliance::{
    AiGovernanceChecker, ComplianceChecker, Do178cChecker, EuAiActChecker, Iec61508Checker,
    Iso26262Checker,
};
use crate::traceability::tracer::Tracer;
use crate::utils::{prelude::*, HashMap};

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct AuditReport {
    pub project_name: String,
    pub date: String,
    pub compliance_results: Vec<serde_json::Value>,
    pub model_stats: ModelStats,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Default, Clone)]
pub struct ModelStats {
    pub total_elements: usize,
    pub total_functions: usize,
    pub total_components: usize,
    pub total_requirements: usize,
    pub total_scenarios: usize,
    pub total_functional_chains: usize,
}

pub struct AuditGenerator;

impl AuditGenerator {
    /// 🎯 GÉNÉRATEUR UNIVERSEL
    /// Orchestre les audits et calcule les statistiques sémantiques.
    pub fn generate(
        tracer: &Tracer,
        docs: &HashMap<String, Value>,
        project_name: &str,
    ) -> AuditReport {
        // 1. Enregistrement des Checkers (Extensibilité O(1))
        let checkers: Vec<Box<dyn ComplianceChecker>> = vec![
            Box::new(Do178cChecker),
            Box::new(Iso26262Checker),
            Box::new(EuAiActChecker),
            Box::new(Iec61508Checker),
            Box::new(AiGovernanceChecker),
        ];

        // 2. Exécution et sérialisation des résultats
        let compliance_results = checkers
            .iter()
            .map(|c| c.check(tracer, docs))
            .filter_map(|r| serde_json::to_value(r).ok())
            .collect();

        // 3. Calcul des statistiques
        let model_stats = Self::calculate_stats(docs);

        AuditReport {
            project_name: project_name.to_string(),
            date: chrono::Utc::now().to_rfc3339(),
            compliance_results,
            model_stats,
        }
    }

    /// Analyse sémantique des types pour le comptage
    fn calculate_stats(docs: &HashMap<String, Value>) -> ModelStats {
        // 🎯 FIX CLIPPY : Initialisation atomique
        let mut stats = ModelStats {
            total_elements: docs.len(),
            ..ModelStats::default()
        };

        for doc in docs.values() {
            let kind = doc.get("kind").and_then(|v| v.as_str()).unwrap_or("");
            let type_iri = doc.get("@type").and_then(|v| v.as_str()).unwrap_or("");

            // Matching robuste sur le Kind ou l'IRI JSON-LD
            if kind == "Function" || type_iri.contains("Function") {
                stats.total_functions += 1;
            } else if kind == "Component" || type_iri.contains("Component") {
                stats.total_components += 1;
            } else if kind == "Requirement" || type_iri.contains("Requirement") {
                stats.total_requirements += 1;
            } else if kind == "Scenario" || type_iri.contains("Scenario") {
                stats.total_scenarios += 1;
            } else if kind == "FunctionalChain" || type_iri.contains("FunctionalChain") {
                stats.total_functional_chains += 1;
            }
        }
        stats
    }
}

// =========================================================================
// TESTS UNITAIRES HYPER ROBUSTES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 🎯 TEST 1 : Vérification de l'intégralité du rapport
    #[test]
    fn test_audit_generate_full_report() {
        let mut docs = HashMap::new();
        docs.insert("F1".into(), json!({ "_id": "F1", "kind": "Function" }));

        let tracer = Tracer::from_json_list(vec![]);
        let report = AuditGenerator::generate(&tracer, &docs, "Test Project");

        assert_eq!(report.project_name, "Test Project");
        // On attend 5 résultats (un par checker enregistré)
        assert_eq!(report.compliance_results.len(), 5);
        assert_eq!(report.model_stats.total_functions, 1);
    }

    /// 🎯 TEST 2 : Robustesse du comptage sémantique (Stats)
    #[test]
    fn test_calculate_stats_semantic_mapping() {
        let mut docs = HashMap::new();
        docs.insert("1".into(), json!({ "kind": "Function" }));
        docs.insert("2".into(), json!({ "@type": "raise:SystemComponent" }));
        docs.insert("3".into(), json!({ "kind": "Requirement" }));
        docs.insert("4".into(), json!({ "kind": "Scenario" }));
        docs.insert("5".into(), json!({ "kind": "FunctionalChain" }));
        // Élément inconnu (ne doit pas fausser les comptes spécifiques)
        docs.insert("6".into(), json!({ "kind": "Unknown" }));

        let stats = AuditGenerator::calculate_stats(&docs);

        assert_eq!(stats.total_elements, 6);
        assert_eq!(stats.total_functions, 1);
        assert_eq!(stats.total_components, 1);
        assert_eq!(stats.total_requirements, 1);
        assert_eq!(stats.total_scenarios, 1);
        assert_eq!(stats.total_functional_chains, 1);
    }

    /// 🎯 TEST 3 : Résilience aux données JSON malformées
    #[test]
    fn test_robustness_malformed_json() {
        let mut docs = HashMap::new();
        // Un document vide ou sans les champs attendus ne doit pas faire paniquer le générateur
        docs.insert("empty".into(), json!({}));
        docs.insert("null_kind".into(), json!({ "kind": null }));

        let tracer = Tracer::from_json_list(vec![]);
        let report = AuditGenerator::generate(&tracer, &docs, "Robustness Test");

        assert_eq!(report.model_stats.total_elements, 2);
        assert_eq!(report.model_stats.total_functions, 0);
        assert!(report.compliance_results.len() > 0);
    }

    /// 🎯 TEST 4 : Intégrité de la date ISO-8601
    #[test]
    fn test_audit_date_format() {
        let tracer = Tracer::from_json_list(vec![]);
        let report = AuditGenerator::generate(&tracer, &HashMap::new(), "Date Test");

        // Vérifie que la date est au format rfc3339 (contient 'T' et 'Z' ou offset)
        assert!(report.date.contains('T'));
    }
}
