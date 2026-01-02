use crate::model_engine::types::ProjectModel;
use crate::traceability::compliance::{
    do_178c::Do178cChecker, eu_ai_act::EuAiActChecker, iso_26262::Iso26262Checker,
    ComplianceChecker,
};
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
}

pub struct AuditGenerator;

impl AuditGenerator {
    pub fn generate(model: &ProjectModel) -> AuditReport {
        // Liste des checkers actifs
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

        AuditReport {
            project_name: model.meta.name.clone(),
            date: chrono::Utc::now().to_rfc3339(),
            compliance_results: results,
            model_stats: ModelStats {
                total_elements: model.meta.element_count,
                total_functions: model.sa.functions.len()
                    + model.la.functions.len()
                    + model.pa.functions.len(),
                total_components: model.sa.components.len()
                    + model.la.components.len()
                    + model.pa.components.len(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectMeta, ProjectModel}; // [CORRECTION] Import NameType
    use std::collections::HashMap;
    // [CORRECTION] Suppression de unused import `serde_json::json`

    // [CORRECTION] Helper robuste sans Default
    fn create_dummy_element(id: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            // [CORRECTION] Envelopper dans NameType::String
            name: NameType::String(format!("Elem {}", id)),
            // [CORRECTION] Ajout champ kind
            kind: "DummyElement".to_string(),
            properties: HashMap::new(),
            // Suppression de ..Default::default()
        }
    }

    #[test]
    fn test_audit_generation_stats() {
        let mut model = ProjectModel::default();

        // Setup Metadonnées
        model.meta = ProjectMeta {
            name: "Projet Test RAISE".to_string(),
            element_count: 5, // Simulé
            ..Default::default()
        };

        // Setup Éléments pour les statistiques
        // 2 Fonctions SA, 1 Composant SA, 1 Composant PA
        model.sa.functions = vec![create_dummy_element("f1"), create_dummy_element("f2")];
        model.sa.components = vec![create_dummy_element("c1")];
        model.pa.components = vec![create_dummy_element("pc1")];

        // Génération
        let report = AuditGenerator::generate(&model);

        // 1. Vérification des infos générales
        assert_eq!(report.project_name, "Projet Test RAISE");
        assert!(!report.date.is_empty());

        // 2. Vérification des statistiques calculées
        // Total functions = 2 (SA) + 0 (LA) + 0 (PA)
        assert_eq!(report.model_stats.total_functions, 2);
        // Total components = 1 (SA) + 0 (LA) + 1 (PA)
        assert_eq!(report.model_stats.total_components, 2);
        // Total elements (vient des métadonnées brutes dans ce mock)
        assert_eq!(report.model_stats.total_elements, 5);

        // 3. Vérification que les checkers ont tourné
        // AuditGenerator hardcode 3 checkers (DO-178C, ISO-26262, EU AI Act)
        assert_eq!(report.compliance_results.len(), 3);

        let first_result = &report.compliance_results[0];
        assert!(first_result.get("standard").is_some());
        assert!(first_result.get("passed").is_some());
    }
}
