// FICHIER : src-tauri/src/traceability/compliance/ai_governance.rs
use crate::utils::prelude::*;

use crate::model_engine::types::{ArcadiaElement, NameType};
use crate::traceability::tracer::Tracer;

#[derive(Debug, Serialize, PartialEq)]
pub struct AiComplianceReport {
    pub component_id: String,
    pub component_name: String,
    pub has_quality_report: bool,
    pub has_xai_frame: bool,
    pub is_compliant: bool,
    pub issues: Vec<String>,
}

pub struct AiGovernanceChecker<'a> {
    tracer: &'a Tracer<'a>,
}

impl<'a> AiGovernanceChecker<'a> {
    pub fn new(tracer: &'a Tracer<'a>) -> Self {
        Self { tracer }
    }

    pub fn audit_element(&self, element: &ArcadiaElement) -> Option<AiComplianceReport> {
        let is_ai = element
            .properties
            .get("nature")
            .and_then(|v| v.as_str())
            .map(|s| s == "AI_Model")
            .unwrap_or(false);

        if !is_ai {
            return None;
        }

        let component_name = match &element.name {
            NameType::String(s) => s.clone(),
            _ => "Nom Inconnu".to_string(),
        };

        let mut report = AiComplianceReport {
            component_id: element.id.clone(),
            component_name,
            has_quality_report: false,
            has_xai_frame: false,
            is_compliant: false,
            issues: Vec::new(),
        };

        // On récupère les preuves liées via model_id (Reverse Link)
        let upstream_docs = self.tracer.get_upstream_elements(&element.id);

        for doc in upstream_docs {
            match doc.kind.as_str() {
                "QualityReport" => {
                    let status = doc
                        .properties
                        .get("overall_status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Fail");

                    if status == "Pass" {
                        report.has_quality_report = true;
                    } else {
                        report
                            .issues
                            .push(format!("Quality Report {} is {}", doc.id, status));
                    }
                }
                "XaiFrame" => {
                    report.has_xai_frame = true;
                }
                _ => {}
            }
        }

        if !report.has_quality_report {
            report
                .issues
                .push("Missing valid Quality Report".to_string());
        }
        if !report.has_xai_frame {
            report
                .issues
                .push("Missing Explainability (XAI) Frame".to_string());
        }

        report.is_compliant = report.issues.is_empty();
        Some(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
    use crate::traceability::tracer::Tracer;
    use crate::utils::{data::json, HashMap};

    fn create_mock_element(id: &str, kind: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(id.to_string()),
            kind: kind.to_string(),
            // CORRECTION : Initialisation du champ description ajouté récemment
            description: None,
            properties,
        }
    }

    #[test]
    fn test_audit_compliant_ai() {
        let ai_model = create_mock_element("ai_v1", "Component", json!({ "nature": "AI_Model" }));

        // Les preuves pointent vers ai_v1 via model_id
        let quality_rep = create_mock_element(
            "q1",
            "QualityReport",
            json!({ "model_id": "ai_v1", "overall_status": "Pass" }),
        );
        let xai_frame = create_mock_element("x1", "XaiFrame", json!({ "model_id": "ai_v1" }));

        let mut model = ProjectModel::default();
        model.pa.components = vec![ai_model.clone(), quality_rep, xai_frame];

        let tracer = Tracer::new(&model); // L'indexation doit maintenant trouver les liens
        let checker = AiGovernanceChecker::new(&tracer);

        let report = checker
            .audit_element(&ai_model)
            .expect("Cible non identifiée");
        assert!(
            report.is_compliant,
            "Le modèle devrait être conforme avec ses preuves"
        );
    }

    #[test]
    fn test_audit_fail_on_quality() {
        let ai_model = create_mock_element("ai_fail", "Component", json!({ "nature": "AI_Model" }));
        let quality_rep = create_mock_element(
            "q_fail",
            "QualityReport",
            json!({ "model_id": "ai_fail", "overall_status": "Fail" }),
        );

        let mut model = ProjectModel::default();
        model.pa.components = vec![ai_model.clone(), quality_rep];

        let tracer = Tracer::new(&model);
        let checker = AiGovernanceChecker::new(&tracer);

        let report = checker.audit_element(&ai_model).unwrap();
        assert!(!report.is_compliant);
        assert!(report.issues.iter().any(|i| i.contains("is Fail")));
    }
}
