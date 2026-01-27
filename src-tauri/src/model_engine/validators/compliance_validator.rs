// FICHIER : src-tauri/src/model_engine/validators/compliance_validator.rs

use crate::model_engine::arcadia; // <-- Import Vocabulaire
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::model_engine::validators::{ModelValidator, Severity, ValidationIssue};
use async_trait::async_trait;

/// Validateur de conformité méthodologique.
#[derive(Default)]
pub struct ComplianceValidator;

impl ComplianceValidator {
    pub fn new() -> Self {
        Self
    }

    /// Vérifie la qualité d'un élément unique
    fn check_quality(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let name = element.name.as_str().trim();

        if name.is_empty() || name == "Unnamed" || name.starts_with("Copy of") {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                element_id: element.id.clone(),
                message: format!("Élément mal nommé ('{}').", name),
                rule_id: "RULE_NAMING".to_string(),
            });
        }

        let has_desc = element
            .description
            .as_ref()
            .map(|d| !d.is_empty())
            .unwrap_or(false);

        if !has_desc {
            issues.push(ValidationIssue {
                severity: Severity::Info,
                element_id: element.id.clone(),
                message: format!("Description manquante pour '{}'", name),
                rule_id: "RULE_DOC_MISSING".to_string(),
            });
        }

        issues
    }

    /// Vérifie les allocations
    fn check_allocations(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let k = &element.kind;

        // Détection générique "Est-ce un composant ?" via le vocabulaire ou substring
        let is_component = k.contains("Component")
            || k == arcadia::KIND_LA_COMPONENT
            || k == arcadia::KIND_SA_COMPONENT
            || k == arcadia::KIND_PA_COMPONENT;

        // Détection "Est-ce un acteur ?"
        let is_actor = k.contains("Actor")
            || k == arcadia::KIND_OA_ACTOR
            || k == arcadia::KIND_SA_ACTOR
            || k == arcadia::KIND_LA_ACTOR
            || k == arcadia::KIND_PA_ACTOR;

        if is_component && !is_actor {
            // Utilisation de la constante pour la clé de propriété
            let has_functions =
                if let Some(val) = element.properties.get(arcadia::PROP_ALLOCATED_FUNCTIONS) {
                    val.as_array().map(|arr| !arr.is_empty()).unwrap_or(false)
                } else {
                    // Fallback legacy key
                    element
                        .properties
                        .get("ownedFunctionalAllocation")
                        .and_then(|v| v.as_array())
                        .map(|a| !a.is_empty())
                        .unwrap_or(false)
                };

            if !has_functions {
                let is_node = k.contains("Node"); // Spécifique PA
                let severity = if is_node {
                    Severity::Info
                } else {
                    Severity::Warning
                };

                issues.push(ValidationIssue {
                    severity,
                    element_id: element.id.clone(),
                    message: format!(
                        "Composant '{}' sans fonction allouée.",
                        element.name.as_str()
                    ),
                    rule_id: "RULE_EMPTY_COMPONENT".to_string(),
                });
            }
        }
        issues
    }

    /// Helper asynchrone pour valider une liste d'éléments.
    async fn validate_list(
        &self,
        elements: &[ArcadiaElement],
        loader: &ModelLoader<'_>,
        issues: &mut Vec<ValidationIssue>,
    ) {
        for el in elements {
            let el_issues = self.validate_element(el, loader).await;
            issues.extend(el_issues);
        }
    }
}

#[async_trait]
impl ModelValidator for ComplianceValidator {
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        _loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        let mut issues = self.check_quality(element);
        issues.extend(self.check_allocations(element));
        issues
    }

    async fn validate_full(&self, loader: &ModelLoader<'_>) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        if let Ok(model) = loader.load_full_model().await {
            // Utilisation systématique du helper
            // OA
            self.validate_list(&model.oa.actors, loader, &mut all_issues)
                .await;
            self.validate_list(&model.oa.activities, loader, &mut all_issues)
                .await;
            self.validate_list(&model.oa.capabilities, loader, &mut all_issues)
                .await;
            self.validate_list(&model.oa.entities, loader, &mut all_issues)
                .await;
            self.validate_list(&model.oa.exchanges, loader, &mut all_issues)
                .await;

            // SA
            self.validate_list(&model.sa.components, loader, &mut all_issues)
                .await;
            self.validate_list(&model.sa.functions, loader, &mut all_issues)
                .await;
            self.validate_list(&model.sa.actors, loader, &mut all_issues)
                .await;
            self.validate_list(&model.sa.capabilities, loader, &mut all_issues)
                .await;
            self.validate_list(&model.sa.exchanges, loader, &mut all_issues)
                .await;

            // LA
            self.validate_list(&model.la.components, loader, &mut all_issues)
                .await;
            self.validate_list(&model.la.functions, loader, &mut all_issues)
                .await;
            self.validate_list(&model.la.actors, loader, &mut all_issues)
                .await;
            self.validate_list(&model.la.interfaces, loader, &mut all_issues)
                .await;
            self.validate_list(&model.la.exchanges, loader, &mut all_issues)
                .await;

            // PA
            self.validate_list(&model.pa.components, loader, &mut all_issues)
                .await;
            self.validate_list(&model.pa.functions, loader, &mut all_issues)
                .await;
            self.validate_list(&model.pa.actors, loader, &mut all_issues)
                .await;
            self.validate_list(&model.pa.links, loader, &mut all_issues)
                .await;
            self.validate_list(&model.pa.exchanges, loader, &mut all_issues)
                .await;

            // EPBS & Data
            self.validate_list(&model.epbs.configuration_items, loader, &mut all_issues)
                .await;
            self.validate_list(&model.data.classes, loader, &mut all_issues)
                .await;
            self.validate_list(&model.data.data_types, loader, &mut all_issues)
                .await;
            self.validate_list(&model.data.exchange_items, loader, &mut all_issues)
                .await;
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

    fn setup_loader() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[tokio::test]
    async fn test_naming_validation_unit() {
        let (_dir, config) = setup_loader();
        let storage = StorageEngine::new(config);
        let loader = ModelLoader::new_with_manager(
            crate::json_db::collections::manager::CollectionsManager::new(&storage, "t", "d"),
        );

        let validator = ComplianceValidator::new();

        let bad_el = ArcadiaElement {
            id: "1".into(),
            name: NameType::String("Unnamed".into()),
            kind: arcadia::KIND_LA_COMPONENT.into(), // Utilisation constante
            description: Some("Desc".into()),
            properties: HashMap::new(),
        };
        let issues = validator.validate_element(&bad_el, &loader).await;
        assert!(issues.iter().any(|i| i.rule_id == "RULE_NAMING"));
    }
}
