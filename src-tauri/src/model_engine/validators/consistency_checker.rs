use super::{ModelValidator, Severity, ValidationIssue};
use crate::model_engine::types::{ArcadiaElement, ProjectModel};

/// Validateur de cohérence technique.
#[derive(Default)]
pub struct ConsistencyChecker;

impl ConsistencyChecker {
    pub fn new() -> Self {
        Self
    }

    /// Méthode unitaire : Valide un seul élément (Votre logique d'origine)
    pub fn validate_element(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let name = element.name.as_str();

        // RÈGLE 1 : Vérification de l'ID
        if element.id.trim().is_empty() {
            issues.push(ValidationIssue {
                severity: Severity::Error, // Critical -> Error
                rule_id: "SYS_001".to_string(),
                element_id: "unknown".to_string(),
                message: format!("L'élément '{}' n'a pas d'identifiant unique (UUID).", name),
            });
        }

        // RÈGLE 2 : Vérification du Nom
        if name.trim().is_empty() || name == "Sans nom" {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                rule_id: "SYS_002".to_string(),
                element_id: element.id.clone(),
                message: "L'élément n'a pas de nom défini.".to_string(),
            });
        }

        // RÈGLE 3 : Vérification du Type
        if element.kind.trim().is_empty() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                rule_id: "SYS_003".to_string(),
                element_id: element.id.clone(),
                message: "Le type de l'élément (URI) est manquant.".to_string(),
            });
        }

        // RÈGLE 4 : Conventions de nommage (Soft check)
        // Les composants devraient commencer par une majuscule (PascalCase)
        if element.kind.contains("Component") || element.kind.contains("Actor") {
            if let Some(first_char) = name.chars().next() {
                if first_char.is_lowercase() {
                    issues.push(ValidationIssue {
                        severity: Severity::Info,
                        rule_id: "NAMING_001".to_string(),
                        element_id: element.id.clone(),
                        message: format!(
                            "Le composant '{}' devrait commencer par une majuscule.",
                            name
                        ),
                    });
                }
            }
        }

        issues
    }
}

// Implémentation du contrat standard pour le moteur
impl ModelValidator for ConsistencyChecker {
    fn validate(&self, model: &ProjectModel) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        // Helper pour parcourir une liste
        let mut check_list = |elements: &[ArcadiaElement]| {
            for el in elements {
                all_issues.extend(self.validate_element(el));
            }
        };

        // 1. Operational Analysis
        check_list(&model.oa.actors);
        check_list(&model.oa.activities);
        check_list(&model.oa.capabilities);
        check_list(&model.oa.entities);

        // 2. System Analysis
        check_list(&model.sa.components);
        check_list(&model.sa.functions);
        check_list(&model.sa.actors);
        check_list(&model.sa.capabilities);

        // 3. Logical Architecture
        check_list(&model.la.components);
        check_list(&model.la.functions);
        check_list(&model.la.actors);

        // 4. Physical Architecture
        check_list(&model.pa.components);
        check_list(&model.pa.functions);
        check_list(&model.pa.actors);

        // 5. EPBS & Data
        check_list(&model.epbs.configuration_items);
        check_list(&model.data.classes);

        all_issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use std::collections::HashMap;

    // Helper pour créer des éléments de test
    fn create_dummy_element(id: &str, name: &str, kind: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(name.to_string()),
            kind: kind.to_string(),
            description: None, // CORRECTION : Initialisation du champ manquant
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_valid_element() {
        let checker = ConsistencyChecker::new();
        let el = create_dummy_element("UUID-1", "MyComponent", "LogicalComponent");
        let issues = checker.validate_element(&el);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_missing_name_warning() {
        let checker = ConsistencyChecker::new();
        let el = create_dummy_element("UUID-2", "", "LogicalComponent");
        let issues = checker.validate_element(&el);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].rule_id, "SYS_002");
    }

    #[test]
    fn test_naming_convention_info() {
        let checker = ConsistencyChecker::new();
        // Nom en minuscule pour un Composant -> Info
        let el = create_dummy_element("UUID-3", "myComponent", "LogicalComponent");
        let issues = checker.validate_element(&el);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Info);
        assert_eq!(issues[0].rule_id, "NAMING_001");
    }

    #[test]
    fn test_full_model_validation() {
        let checker = ConsistencyChecker::new();
        let mut model = ProjectModel::default();

        // Ajout d'un élément invalide dans le modèle
        let bad_el = create_dummy_element("UUID-Bad", "", "SystemFunction");
        model.sa.functions.push(bad_el);

        let issues = checker.validate(&model);
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "SYS_002");
    }
}
