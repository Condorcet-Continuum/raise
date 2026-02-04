// FICHIER : src-tauri/src/traceability/reporting/audit_report.rs

use crate::model_engine::types::ProjectModel;
use crate::traceability::compliance::{
    ai_governance::AiGovernanceChecker, do_178c::Do178cChecker, eu_ai_act::EuAiActChecker,
    iso_26262::Iso26262Checker, ComplianceChecker, ComplianceReport, Violation,
};
use crate::traceability::tracer::Tracer;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct AuditReport {
    pub project_name: String,
    pub date: String,
    pub compliance_results: Vec<serde_json::Value>,
    pub model_stats: ModelStats,
}

#[derive(Debug, Serialize)]
pub struct ModelStats {
    pub total_elements: usize,
    pub total_functions: usize,
    pub total_components: usize,
    // AJOUT : Statistiques de la couche Transverse
    pub total_requirements: usize,
    pub total_scenarios: usize,
    pub total_functional_chains: usize,
}

pub struct AuditGenerator;

impl AuditGenerator {
    pub fn generate(model: &ProjectModel) -> AuditReport {
        // 1. Exécution des Checkers Standards (Approche globale)
        let checkers: Vec<Box<dyn ComplianceChecker>> = vec![
            Box::new(Do178cChecker),
            Box::new(Iso26262Checker),
            Box::new(EuAiActChecker),
        ];

        let mut results = Vec::new();
        for checker in checkers {
            let report = checker.check(model);
            results.push(serde_json::to_value(report).unwrap());
        }

        // 2. Exécution de l'Audit IA (Approche spécifique via Tracer)
        let ai_report = Self::run_ai_audit(model);
        results.push(serde_json::to_value(ai_report).unwrap());

        // 3. Synthèse
        AuditReport {
            project_name: model.meta.name.clone(),
            date: chrono::Utc::now().to_rfc3339(),
            compliance_results: results,
            model_stats: ModelStats {
                total_elements: model.meta.element_count,
                // Somme des fonctions (SA/LA/PA)
                total_functions: model.sa.functions.len()
                    + model.la.functions.len()
                    + model.pa.functions.len(),
                // Somme des composants (SA/LA/PA)
                total_components: model.sa.components.len()
                    + model.la.components.len()
                    + model.pa.components.len(),
                // AJOUT : Comptage Transverse
                total_requirements: model.transverse.requirements.len(),
                total_scenarios: model.transverse.scenarios.len(),
                total_functional_chains: model.transverse.functional_chains.len(),
            },
        }
    }

    /// Logique spécifique pour scanner les modèles IA et vérifier leur conformité
    fn run_ai_audit(model: &ProjectModel) -> ComplianceReport {
        let tracer = Tracer::new(model);
        let ai_checker = AiGovernanceChecker::new(&tracer);

        let mut violations = Vec::new();
        let mut checked_count = 0;

        // On scanne les composants de l'architecture physique (PA)
        for component in &model.pa.components {
            // L'auditeur renvoie Some(...) seulement si c'est un composant IA
            if let Some(report) = ai_checker.audit_element(component) {
                checked_count += 1;

                if !report.is_compliant {
                    for issue in report.issues {
                        violations.push(Violation {
                            element_id: Some(report.component_id.clone()),
                            rule_id: "AI-GOV-CHECK".to_string(),
                            description: format!(
                                "Composant IA '{}' non conforme : {}",
                                report.component_name, issue
                            ),
                            severity: "Critical".to_string(),
                        });
                    }
                }
            }
        }

        ComplianceReport {
            standard: "RAISE AI Governance".to_string(),
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
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectMeta, ProjectModel};
    use serde_json::json;
    use std::collections::HashMap;

    fn create_dummy_element(id: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(format!("Elem {}", id)),
            kind: "Dummy".to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    // Helper: Création d'un composant tagué comme IA
    fn create_ai_component(id: &str) -> ArcadiaElement {
        let mut props = HashMap::new();
        props.insert("nature".to_string(), json!("AI_Model"));
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(format!("AI {}", id)),
            kind: "Component".to_string(),
            description: None,
            properties: props,
        }
    }

    #[test]
    fn test_audit_generator_structure_with_transverse() {
        let mut model = ProjectModel::default();
        model.meta = ProjectMeta {
            name: "Projet Test".to_string(),
            element_count: 10,
            ..Default::default()
        };
        // Ajout d'éléments classiques
        model.sa.functions = vec![create_dummy_element("f1")];

        // Ajout d'éléments Transverses
        model
            .transverse
            .requirements
            .push(create_dummy_element("REQ-1"));
        model
            .transverse
            .scenarios
            .push(create_dummy_element("SCEN-1"));

        let report = AuditGenerator::generate(&model);

        assert_eq!(report.project_name, "Projet Test");
        assert_eq!(report.model_stats.total_functions, 1);

        // Assertions Transverses
        assert_eq!(
            report.model_stats.total_requirements, 1,
            "Compte exigences incorrect"
        );
        assert_eq!(
            report.model_stats.total_scenarios, 1,
            "Compte scénarios incorrect"
        );
        assert_eq!(report.model_stats.total_functional_chains, 0); // Pas ajouté
    }

    #[test]
    fn test_audit_detects_ai_issues() {
        let mut model = ProjectModel::default();

        // On insère un modèle IA "nu" (sans preuve de qualité/XAI)
        let ai_comp = create_ai_component("ai_vision");
        model.pa.components = vec![ai_comp];

        let report = AuditGenerator::generate(&model);

        // On cherche la section AI Governance
        let governance_result = report
            .compliance_results
            .iter()
            .find(|r| r["standard"] == "RAISE AI Governance")
            .expect("Le rapport de gouvernance IA est manquant");

        // Il doit être en échec car pas de preuves
        assert_eq!(governance_result["passed"], false);

        let violations = governance_result["violations"].as_array().unwrap();
        assert!(!violations.is_empty());
        assert!(violations[0]["description"]
            .as_str()
            .unwrap()
            .contains("Missing valid Quality Report"));
    }
}
