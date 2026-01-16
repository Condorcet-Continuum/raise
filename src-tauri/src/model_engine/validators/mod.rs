// FICHIER : src-tauri/src/model_engine/validators/mod.rs

pub mod compliance_validator;
pub mod consistency_checker;
pub mod dynamic_validator;

use crate::model_engine::types::ProjectModel;
use serde::{Deserialize, Serialize};

// Re-exports pour faciliter l'usage externe
pub use compliance_validator::ComplianceValidator;
pub use consistency_checker::ConsistencyChecker;
pub use dynamic_validator::DynamicValidator;

/// Niveau de sévérité d'un problème de validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,   // Bloquant / Rouge
    Warning, // Avertissement / Jaune
    Info,    // Suggestion / Bleu
}

/// Représente un problème détecté dans le modèle.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub rule_id: String,
    pub element_id: String,
    pub message: String,
}

/// Trait commun que tous les validateurs doivent implémenter.
pub trait ModelValidator {
    fn validate(&self, model: &ProjectModel) -> Vec<ValidationIssue>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    // Imports nécessaires pour les tests d'intégration
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::rules_engine::{Expr, Rule};

    struct MockValidator;
    impl ModelValidator for MockValidator {
        fn validate(&self, _model: &ProjectModel) -> Vec<ValidationIssue> {
            vec![ValidationIssue {
                severity: Severity::Error,
                rule_id: "MOCK_RULE".to_string(),
                element_id: "mock_id".to_string(),
                message: "Mock Error".to_string(),
            }]
        }
    }

    #[test]
    fn test_severity_serialization() {
        assert_eq!(
            serde_json::to_value(Severity::Error).unwrap(),
            json!("Error")
        );
    }

    #[test]
    fn test_validation_issue_structure() {
        let issue = ValidationIssue {
            severity: Severity::Warning,
            rule_id: "TEST_001".to_string(),
            element_id: "uuid-123".to_string(),
            message: "Something is wrong".to_string(),
        };
        let json = serde_json::to_value(&issue).unwrap();
        assert_eq!(json["severity"], "Warning");
        assert_eq!(json["rule_id"], "TEST_001");
    }

    #[test]
    fn test_trait_implementation() {
        let model = ProjectModel::default();
        let validator = MockValidator;
        let issues = validator.validate(&model);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "Mock Error");
    }

    #[test]
    fn test_dynamic_validator_integration() {
        // 1. Création d'une règle via l'AST
        let rule_expr = Expr::Eq(vec![
            Expr::Var("name".to_string()),
            Expr::Val(json!("ValidElement")),
        ]);

        let rule = Rule {
            id: "INTEGRATION_TEST_RULE".to_string(),
            target: "oa.actors".to_string(),
            expr: rule_expr,
            // CORRECTION : Champs obligatoires ajoutés
            description: Some("Test integration".to_string()),
            severity: Some("Warning".to_string()),
        };

        // 2. Instanciation du Validateur via le Trait
        let validator: Box<dyn ModelValidator> = Box::new(DynamicValidator::new(vec![rule]));

        // 3. Création du Modèle
        let mut model = ProjectModel::default();

        let mut actor1 = ArcadiaElement::default();
        actor1.name = NameType::String("ValidElement".to_string());
        model.oa.actors.push(actor1);

        let mut actor2 = ArcadiaElement::default();
        actor2.name = NameType::String("InvalidElement".to_string());
        model.oa.actors.push(actor2);

        // 4. Exécution
        let issues = validator.validate(&model);

        // 5. Vérification
        assert_eq!(issues.len(), 1, "Il devrait y avoir exactement 1 erreur");
        assert_eq!(issues[0].rule_id, "INTEGRATION_TEST_RULE");
        assert_eq!(issues[0].element_id, model.oa.actors[1].id);
    }
}
