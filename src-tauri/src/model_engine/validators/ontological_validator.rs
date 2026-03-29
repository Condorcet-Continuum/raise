// FICHIER : src-tauri/src/model_engine/validators/ontological_validator.rs

use super::{ModelValidator, Severity, ValidationIssue};
use crate::model_engine::arcadia::ArcadiaOntology;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::utils::prelude::*;

#[derive(Default)]
pub struct OntologicalValidator;

impl OntologicalValidator {
    pub fn new() -> Self {
        Self
    }

    /// Logique de validation pilotée par les données (Data-Driven)
    pub fn check_semantics(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let kind = element.kind.as_str();

        // "Unknown" est toléré temporairement en cours de modélisation (SysML incomplet)
        if kind != "Unknown" && !ArcadiaOntology::is_known_type(kind) {
            issues.push(ValidationIssue {
                rule_id: "ONTO-001".to_string(),
                severity: Severity::Warning,
                element_id: element.id.clone(),
                message: format!(
                    "Sémantique inconnue ou non-mappée dans l'ontologie : '{}'",
                    kind
                ),
            });
        }

        issues
    }
}

#[async_interface]
impl ModelValidator for OntologicalValidator {
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        _loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        self.check_semantics(element)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;

    #[test]
    fn test_ontological_validation() {
        let validator = OntologicalValidator::new();

        // 🎯 On teste avec l'état "Unknown" (Brouillon en cours de modélisation)
        // C'est la règle métier qui permet de contourner le registre strict.
        let valid_draft = ArcadiaElement {
            id: "draft-001".to_string(),
            name: NameType::String("Brouillon".to_string()),
            kind: "Unknown".to_string(),
            ..Default::default()
        };

        let issues = validator.check_semantics(&valid_draft);
        assert_eq!(
            issues.len(),
            0,
            "Un brouillon ('Unknown') ne doit pas lever d'erreur ontologique"
        );

        // 🎯 On teste avec un type inventé (qui sera rejeté par le mock vide)
        let invalid_element = ArcadiaElement {
            id: "err-001".to_string(),
            name: NameType::String("Erreur Magique".to_string()),
            kind: "MagicalEntity".to_string(),
            ..Default::default()
        };

        let issues_err = validator.check_semantics(&invalid_element);
        assert_eq!(
            issues_err.len(),
            1,
            "Une erreur doit être levée pour un type inexistant"
        );
        assert_eq!(issues_err[0].rule_id, "ONTO-001");
    }
}
