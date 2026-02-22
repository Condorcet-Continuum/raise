// FICHIER : src-tauri/src/model_engine/validators/compliance_validator.rs
use crate::utils::async_trait;

use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::model_engine::validators::{ModelValidator, Severity, ValidationIssue};

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

        // Règle 1 : Nommage explicite
        if name.is_empty() || name == "Unnamed" || name.starts_with("Copy of") {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                element_id: element.id.clone(),
                message: format!("Élément mal nommé ('{}').", name),
                rule_id: "RULE_NAMING".to_string(),
            });
        }

        // Règle 2 : Description présente
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
                rule_id: "RULE_DOC".to_string(),
            });
        }

        issues
    }

    /// Helper pour itérer sur une liste
    async fn validate_list(
        &self,
        elements: &[ArcadiaElement],
        _loader: &ModelLoader<'_>,
        issues: &mut Vec<ValidationIssue>,
    ) {
        for el in elements {
            issues.extend(self.check_quality(el));
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
        self.check_quality(element)
    }

    async fn validate_full(&self, loader: &ModelLoader<'_>) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        if let Ok(model) = loader.load_full_model().await {
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

            // AJOUT : COUCHE TRANSVERSE
            self.validate_list(&model.transverse.requirements, loader, &mut all_issues)
                .await;
            self.validate_list(&model.transverse.scenarios, loader, &mut all_issues)
                .await;
            self.validate_list(&model.transverse.functional_chains, loader, &mut all_issues)
                .await;
            self.validate_list(&model.transverse.constraints, loader, &mut all_issues)
                .await;
            self.validate_list(
                &model.transverse.common_definitions,
                loader,
                &mut all_issues,
            )
            .await;
            self.validate_list(&model.transverse.others, loader, &mut all_issues)
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
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::{data::HashMap, io::tempdir};

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
            // CORRECTION : Utilisation d'une chaîne directe pour éviter l'import inutilisé arcadia::KIND_...
            kind: "LogicalComponent".into(),
            description: Some("Desc".into()),
            properties: HashMap::new(),
        };
        let issues = validator.validate_element(&bad_el, &loader).await;

        assert!(!issues.is_empty());
        assert_eq!(issues[0].rule_id, "RULE_NAMING");
    }

    #[tokio::test]
    async fn test_compliance_transverse_naming() {
        inject_mock_config();

        // Vérifie que les règles s'appliquent aussi aux éléments transverses
        let (_dir, config) = setup_loader();
        let storage = StorageEngine::new(config);
        let manager =
            crate::json_db::collections::manager::CollectionsManager::new(&storage, "space", "db");
        manager.init_db().await.unwrap();

        // Insertion d'un Requirement mal nommé
        let req = serde_json::json!({
            "id": "REQ-BAD",
            "name": "Copy of Req 1", // Trigger RULE_NAMING
            "type": "Requirement"
        });
        manager.insert_raw("transverse", &req).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        let validator = ComplianceValidator::new();

        let issues = validator.validate_full(&loader).await;

        let found = issues
            .iter()
            .any(|i| i.element_id == "REQ-BAD" && i.rule_id == "RULE_NAMING");
        assert!(
            found,
            "Le validateur de conformité ignore la couche Transverse !"
        );
    }
}
