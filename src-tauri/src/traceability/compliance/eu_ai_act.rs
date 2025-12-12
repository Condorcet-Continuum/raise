use super::{ComplianceChecker, ComplianceReport, Violation};
use crate::model_engine::types::ProjectModel;

pub struct EuAiActChecker;

impl ComplianceChecker for EuAiActChecker {
    fn name(&self) -> &str {
        "EU AI Act (Transparency & Record-keeping)"
    }

    fn check(&self, model: &ProjectModel) -> ComplianceReport {
        let mut violations = Vec::new();
        let mut rules_checked = 0;

        // Règle 1 : Transparence des modèles IA
        // Tout composant PA marqué comme "AI_Model" doit référencer une preuve XAI récente.
        rules_checked += 1;

        for comp in &model.pa.components {
            // On vérifie si le composant est tagué comme étant de l'IA
            // Convention: propriété "component_type" = "AI_Model"
            let is_ai = comp
                .properties
                .get("component_type")
                .and_then(|v| v.as_str())
                .map(|t| t == "AI_Model")
                .unwrap_or(false);

            if is_ai {
                // Vérification de la présence de la référence XAI
                let has_evidence = comp.properties.contains_key("xai_evidence_ref");

                if !has_evidence {
                    violations.push(Violation {
                        element_id: Some(comp.id.clone()),
                        rule_id: "AI-ACT-TRANS-01".into(),
                        description: format!(
                            "Le modèle IA '{}' n'a aucune trame d'explicabilité (XAI) associée.",
                            comp.name.as_str() // [CORRECTION] Utilisation de .as_str() pour NameType
                        ),
                        severity: "Critical".into(),
                    });
                }
            }
        }

        ComplianceReport {
            standard: self.name().to_string(),
            passed: violations.is_empty(),
            rules_checked,
            violations,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel}; // [CORRECTION] Import de NameType
    use serde_json::json;
    use std::collections::HashMap;

    // [CORRECTION] Helper adapté : Construit l'objet entièrement sans Default
    fn create_comp(id: &str, name: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }

        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(name.to_string()), // [CORRECTION] Envelopper dans l'Enum
            kind: "Component".to_string(),            // [CORRECTION] Champ 'kind' obligatoire
            properties,
        }
    }

    #[test]
    fn test_ai_act_compliance() {
        let checker = EuAiActChecker;
        let mut model = ProjectModel::default();

        // 1. Composant Standard (Ignoré par le checker)
        let std_comp = create_comp("c1", "Database", json!({ "component_type": "Database" }));

        // 2. Modèle IA Non Conforme (Pas de preuve XAI)
        let ai_bad = create_comp("ai1", "Face Reco", json!({ "component_type": "AI_Model" }));

        // 3. Modèle IA Conforme (Avec preuve XAI)
        let ai_good = create_comp(
            "ai2",
            "Spam Filter",
            json!({
                "component_type": "AI_Model",
                "xai_evidence_ref": "uuid-1234-5678"
            }),
        );

        model.pa.components = vec![std_comp, ai_bad, ai_good];

        let report = checker.check(&model);

        assert!(!report.passed);
        assert_eq!(report.violations.len(), 1);
        assert_eq!(report.violations[0].element_id.as_deref(), Some("ai1"));
        assert_eq!(report.violations[0].rule_id, "AI-ACT-TRANS-01");
    }
}
