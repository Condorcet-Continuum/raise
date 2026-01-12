pub mod compliance_validator;
pub mod consistency_checker;

use crate::model_engine::types::ProjectModel;
use serde::{Deserialize, Serialize};

// Re-exports pour faciliter l'usage externe
pub use compliance_validator::ComplianceValidator;
pub use consistency_checker::ConsistencyChecker;

/// Niveau de sévérité d'un problème de validation.
/// Utilisé pour colorer les alertes dans l'interface utilisateur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Severity {
    Error,   // Bloquant / Rouge
    Warning, // Avertissement / Jaune
    Info,    // Suggestion / Bleu
}

/// Représente un problème détecté dans le modèle.
/// Cette structure est destinée à être sérialisée en JSON pour le frontend.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ValidationIssue {
    pub severity: Severity,
    pub rule_id: String,    // Code unique de la règle (ex: "RULE_ORPHAN")
    pub element_id: String, // ID de l'élément en cause (pour le surlignage)
    pub message: String,    // Message lisible pour l'utilisateur
}

/// Trait commun que tous les validateurs doivent implémenter.
/// Permet d'injecter n'importe quel validateur dans le moteur.
pub trait ModelValidator {
    fn validate(&self, model: &ProjectModel) -> Vec<ValidationIssue>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    // Mock pour tester le trait ModelValidator
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
        // Vérifie que l'Enum se sérialise correctement pour le frontend
        let error = Severity::Error;
        let warning = Severity::Warning;

        assert_eq!(serde_json::to_value(error).unwrap(), json!("Error"));
        assert_eq!(serde_json::to_value(warning).unwrap(), json!("Warning"));
    }

    #[test]
    fn test_severity_deserialization() {
        // Vérifie qu'on peut relire depuis le JSON
        let json_str = "\"Info\"";
        let severity: Severity = serde_json::from_str(json_str).unwrap();
        assert_eq!(severity, Severity::Info);
    }

    #[test]
    fn test_validation_issue_structure() {
        // Vérifie la construction et l'égalité
        let issue = ValidationIssue {
            severity: Severity::Warning,
            rule_id: "TEST_001".to_string(),
            element_id: "uuid-123".to_string(),
            message: "Something is wrong".to_string(),
        };

        let json = serde_json::to_value(&issue).unwrap();

        assert_eq!(json["severity"], "Warning");
        assert_eq!(json["rule_id"], "TEST_001");
        assert_eq!(json["elementId"], json!(null)); // camelCase vs snake_case : par défaut Rust garde snake_case sauf si rename_all
                                                    // Note : Si vous utilisez #[serde(rename_all = "camelCase")] sur la struct, changez ce test.
                                                    // Ici, sans attribut, c'est "element_id".
        assert_eq!(json["element_id"], "uuid-123");
    }

    #[test]
    fn test_trait_implementation() {
        // Vérifie que le trait est utilisable
        let model = ProjectModel::default();
        let validator = MockValidator;

        let issues = validator.validate(&model);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].message, "Mock Error");
    }
}
