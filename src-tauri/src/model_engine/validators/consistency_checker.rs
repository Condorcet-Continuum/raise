// FICHIER : src-tauri/src/model_engine/validators/consistency_checker.rs

use super::{ModelValidator, Severity, ValidationIssue};
use crate::model_engine::arcadia; // <-- Import Vocabulaire
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use async_trait::async_trait;

/// Validateur de cohérence technique.
#[derive(Default)]
pub struct ConsistencyChecker;

impl ConsistencyChecker {
    pub fn new() -> Self {
        Self
    }

    /// Logique de validation unitaire pure
    pub fn check_logic(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let name = element.name.as_str();

        // RÈGLE 1 : Vérification de l'ID
        if element.id.trim().is_empty() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
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

        // RÈGLE 3 : Vérification du Type (URI)
        if element.kind.trim().is_empty() || element.kind == arcadia::KIND_UNKNOWN {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                rule_id: "SYS_003".to_string(),
                element_id: element.id.clone(),
                message: "Le type de l'élément (URI) est manquant ou inconnu.".to_string(),
            });
        }

        // RÈGLE 4 : Conventions de nommage (Soft check)
        let is_component_or_actor =
            element.kind.contains("Component") || element.kind.contains("Actor");

        if is_component_or_actor {
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

#[async_trait]
impl ModelValidator for ConsistencyChecker {
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        _loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        self.check_logic(element)
    }

    async fn validate_full(&self, loader: &ModelLoader<'_>) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        if let Ok(model) = loader.load_full_model().await {
            // Helper local
            let mut check_list = |elements: &[ArcadiaElement]| {
                for el in elements {
                    all_issues.extend(self.check_logic(el));
                }
            };

            // Tous les layers...
            check_list(&model.oa.actors);
            check_list(&model.oa.activities);
            check_list(&model.oa.capabilities);
            check_list(&model.oa.entities);
            check_list(&model.oa.exchanges);

            check_list(&model.sa.components);
            check_list(&model.sa.functions);
            check_list(&model.sa.actors);
            check_list(&model.sa.capabilities);
            check_list(&model.sa.exchanges);

            check_list(&model.la.components);
            check_list(&model.la.functions);
            check_list(&model.la.actors);
            check_list(&model.la.interfaces);
            check_list(&model.la.exchanges);

            check_list(&model.pa.components);
            check_list(&model.pa.functions);
            check_list(&model.pa.actors);
            check_list(&model.pa.links);
            check_list(&model.pa.exchanges);

            check_list(&model.epbs.configuration_items);
            check_list(&model.data.classes);
            check_list(&model.data.data_types);
            check_list(&model.data.exchange_items);
        }

        all_issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn create_dummy_element(id: &str, name: &str, kind: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(name.to_string()),
            kind: kind.to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_valid_element_logic() {
        let checker = ConsistencyChecker::new();
        let el = create_dummy_element("UUID-1", "MyComponent", arcadia::KIND_LA_COMPONENT);
        let issues = checker.check_logic(&el);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_missing_name_warning() {
        let checker = ConsistencyChecker::new();
        let el = create_dummy_element("UUID-2", "", arcadia::KIND_LA_COMPONENT);
        let issues = checker.check_logic(&el);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].rule_id, "SYS_002");
    }

    #[tokio::test]
    async fn test_validate_element_trait() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let loader = ModelLoader::new_with_manager(
            crate::json_db::collections::manager::CollectionsManager::new(&storage, "test", "val"),
        );

        let checker = ConsistencyChecker::new();
        let el = create_dummy_element("UUID-3", "badName", arcadia::KIND_SA_COMPONENT);

        let issues = checker.validate_element(&el, &loader).await;
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].rule_id, "NAMING_001");
    }
}
