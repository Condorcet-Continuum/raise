// FICHIER : src-tauri/src/model_engine/validators/ontological_validator.rs

use super::{ModelValidator, Severity, ValidationIssue};
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use async_trait::async_trait;

#[derive(Default)]
pub struct OntologicalValidator;

impl OntologicalValidator {
    pub fn new() -> Self {
        Self
    }

    /// Logique de validation interne, synchrone et facilement testable.
    /// Vérifie que le type (kind) de l'élément existe bien dans l'ontologie MBSE2.
    pub fn check_semantics(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let kind = element.kind.as_str();

        // Liste des sémantiques acceptées par notre ontologie combinée (Arcadia + SysML2)
        let known_semantics = [
            "OperationalActor",
            "SystemActor",
            "LogicalActor",
            "PhysicalActor",
            "OperationalActivity",
            "SystemFunction",
            "LogicalFunction",
            "PhysicalFunction",
            "Function",
            "SystemComponent",
            "LogicalComponent",
            "PhysicalComponent",
            "ConfigurationItem",
            "Component",
            "Requirement",
            "Constraint",
            "State",
            "DataClass",
            "ExchangeItem",
            "PartDefinition",
            "ActorDefinition",
            "ItemDefinition",
            "ActionDefinition",
            "Unknown", // Laissé pour les éléments en cours de modélisation
        ];

        if !known_semantics.contains(&kind) {
            issues.push(ValidationIssue {
                rule_id: "ONTO-001".to_string(), // <=== CORRECTION : Ajout de rule_id
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

#[async_trait]
impl ModelValidator for OntologicalValidator {
    // <=== CORRECTION : Remplacement de `validate` par l'implémentation asynchrone attendue
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        _loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        // Pour l'instant on délègue à la méthode synchrone
        // Plus tard, on pourrait utiliser _loader pour vérifier que les URIs pointées par
        // l'élément existent vraiment dans la base JSON-LD !
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

        // On teste avec un élément valide
        let valid_actor = ArcadiaElement {
            id: "oa-act-client".to_string(),
            name: NameType::String("Client".to_string()),
            kind: "OperationalActor".to_string(),
            ..Default::default()
        };

        // On teste directement la logique métier sans avoir besoin d'un ModelLoader mocké
        let issues = validator.check_semantics(&valid_actor);
        assert_eq!(issues.len(), 0, "L'acteur doit être ontologiquement valide");

        // On teste avec un élément dont la sémantique n'existe pas dans SysML2/Arcadia
        let invalid_element = ArcadiaElement {
            id: "err-001".to_string(),
            name: NameType::String("Erreur Magique".to_string()),
            kind: "MagicalEntity".to_string(), // <=== Type invalide !
            ..Default::default()
        };

        let issues_err = validator.check_semantics(&invalid_element);
        assert_eq!(issues_err.len(), 1, "Une erreur doit être levée");
        assert_eq!(
            issues_err[0].rule_id, "ONTO-001",
            "La règle ONTO-001 doit être déclenchée"
        );
    }
}
