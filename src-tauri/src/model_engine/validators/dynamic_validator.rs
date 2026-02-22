// FICHIER : src-tauri/src/model_engine/validators/dynamic_validator.rs

use crate::utils::{async_trait, prelude::*};

use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::model_engine::validators::{ModelValidator, Severity, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::Evaluator;

pub struct DynamicValidator {
    rules: Vec<Rule>,
}

impl DynamicValidator {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Vérifie si une règle s'applique à un élément donné via son type (Kind).
    fn rule_applies_to(&self, rule_target: &str, element_kind: &str) -> bool {
        match rule_target {
            // OA
            "oa.actors" => element_kind.contains("OperationalActor"),
            "oa.activities" => element_kind.contains("OperationalActivity"),
            "oa.capabilities" => element_kind.contains("OperationalCapability"),
            "oa.entities" => element_kind.contains("OperationalEntity"),
            // SA
            "sa.components" => element_kind.contains("SystemComponent"),
            "sa.functions" => element_kind.contains("SystemFunction"),
            "sa.actors" => element_kind.contains("SystemActor"),
            // LA
            "la.components" => element_kind.contains("LogicalComponent"),
            "la.functions" => element_kind.contains("LogicalFunction"),
            // PA
            "pa.components" => element_kind.contains("PhysicalComponent"),
            // Transverse
            "transverse.requirements" => element_kind.contains("Requirement"),
            "transverse.scenarios" => element_kind.contains("Scenario"),
            "transverse.functional_chains" => element_kind.contains("FunctionalChain"),
            "transverse.constraints" => element_kind.contains("Constraint"),
            // Generic
            "all" => true,
            _ => false,
        }
    }

    /// Construit le contexte JSON pour l'évaluation de la règle
    fn build_context(element: &ArcadiaElement) -> Value {
        let mut context = json!({
            "id": element.id,
            "name": element.name.as_str(),
            "kind": element.kind,
            "description": element.description
        });

        // Fusion des propriétés dynamiques (flatten)
        if let Some(obj) = context.as_object_mut() {
            for (k, v) in &element.properties {
                if !obj.contains_key(k) {
                    obj.insert(k.clone(), v.clone());
                }
            }
        }
        context
    }

    /// Helper pour valider une liste d'éléments (remplace la closure async problématique)
    async fn validate_list(
        &self,
        elements: &[ArcadiaElement],
        loader: &ModelLoader<'_>,
        issues: &mut Vec<ValidationIssue>,
    ) {
        for el in elements {
            // On réutilise validate_element pour garantir la cohérence
            let el_issues = self.validate_element(el, loader).await;
            issues.extend(el_issues);
        }
    }
}

#[async_trait]
impl ModelValidator for DynamicValidator {
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let context = Self::build_context(element);

        // Filtrage des règles applicables
        let applicable_rules: Vec<&Rule> = self
            .rules
            .iter()
            .filter(|r| self.rule_applies_to(&r.target, &element.kind))
            .collect();

        for rule in applicable_rules {
            // Appel direct à la méthode statique evaluate
            match Evaluator::evaluate(&rule.expr, &context, loader).await {
                Ok(result) => {
                    // Conversion du résultat (Cow<Value>) en booléen
                    let is_valid = match result.as_ref() {
                        Value::Bool(b) => *b,
                        Value::Null => false,
                        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
                        Value::String(s) => !s.is_empty(),
                        _ => true,
                    };

                    if !is_valid {
                        issues.push(ValidationIssue {
                            severity: match rule.severity.as_deref() {
                                Some("Error") => Severity::Error,
                                Some("Info") => Severity::Info,
                                _ => Severity::Warning,
                            },
                            rule_id: rule.id.clone(),
                            element_id: element.id.clone(),
                            message: rule
                                .description
                                .clone()
                                .unwrap_or_else(|| format!("Règle {} non respectée", rule.id)),
                        });
                    }
                }
                Err(e) => {
                    issues.push(ValidationIssue {
                        severity: Severity::Info,
                        rule_id: "EVAL_ERROR".to_string(),
                        element_id: element.id.clone(),
                        message: format!("Erreur d'évaluation règle {}: {}", rule.id, e),
                    });
                }
            }
        }
        issues
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

            // SA
            self.validate_list(&model.sa.components, loader, &mut all_issues)
                .await;
            self.validate_list(&model.sa.functions, loader, &mut all_issues)
                .await;
            self.validate_list(&model.sa.actors, loader, &mut all_issues)
                .await;

            // LA
            self.validate_list(&model.la.components, loader, &mut all_issues)
                .await;
            self.validate_list(&model.la.functions, loader, &mut all_issues)
                .await;

            // PA
            self.validate_list(&model.pa.components, loader, &mut all_issues)
                .await;

            // AJOUT : TRANSVERSE
            self.validate_list(&model.transverse.requirements, loader, &mut all_issues)
                .await;
            self.validate_list(&model.transverse.scenarios, loader, &mut all_issues)
                .await;
            self.validate_list(&model.transverse.functional_chains, loader, &mut all_issues)
                .await;
            self.validate_list(&model.transverse.constraints, loader, &mut all_issues)
                .await;
        }

        all_issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::NameType;
    use crate::rules_engine::ast::Expr;
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::{data::HashMap, io::tempdir};

    fn setup_env() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[tokio::test]
    async fn test_dynamic_rule_application() {
        let (_dir, config) = setup_env();
        let storage = StorageEngine::new(config);
        let loader = ModelLoader::new_with_manager(
            crate::json_db::collections::manager::CollectionsManager::new(&storage, "t", "d"),
        );

        // Règle : name == "ValidElement"
        let rule_expr = Expr::Eq(vec![
            Expr::Var("name".to_string()),
            Expr::Val(json!("ValidElement")),
        ]);

        let rule = Rule {
            id: "TEST_RULE".to_string(),
            target: "oa.actors".to_string(),
            expr: rule_expr,
            description: Some("Nom invalide".to_string()),
            severity: Some("Warning".to_string()),
        };

        let validator = DynamicValidator::new(vec![rule]);

        // Élément valide
        let valid_el = ArcadiaElement {
            id: "1".into(),
            name: NameType::String("ValidElement".into()),
            kind: "OperationalActor".into(),
            description: None,
            properties: HashMap::new(),
        };

        // Élément invalide
        let invalid_el = ArcadiaElement {
            id: "2".into(),
            name: NameType::String("BadName".into()),
            kind: "OperationalActor".into(),
            description: None,
            properties: HashMap::new(),
        };

        let issues_1 = validator.validate_element(&valid_el, &loader).await;
        assert!(issues_1.is_empty());

        let issues_2 = validator.validate_element(&invalid_el, &loader).await;
        assert_eq!(issues_2.len(), 1);
        assert_eq!(issues_2[0].rule_id, "TEST_RULE");
    }

    #[tokio::test]
    async fn test_dynamic_rule_on_transverse_requirement() {
        inject_mock_config();

        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage, "tr_space", "tr_db",
        );
        manager.init_db().await.unwrap();

        // Règle dynamique : properties/priority == "High"
        let rule_expr = Expr::Eq(vec![
            Expr::Var("priority".to_string()),
            Expr::Val(json!("High")),
        ]);

        let rule = Rule {
            id: "REQ_PRIORITY".to_string(),
            target: "transverse.requirements".to_string(),
            expr: rule_expr,
            description: Some("Must be High priority".to_string()),
            severity: Some("Error".to_string()),
        };

        // Création d'une exigence invalide (Low priority)
        let req = json!({
            "id": "REQ-LOW",
            "name": "Slow Request",
            "type": "Requirement",
            "priority": "Low"
        });
        manager.insert_raw("transverse", &req).await.unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        let validator = DynamicValidator::new(vec![rule]);

        let issues = validator.validate_full(&loader).await;

        assert_eq!(
            issues.len(),
            1,
            "La règle dynamique sur l'exigence aurait dû échouer"
        );
        assert_eq!(issues[0].element_id, "REQ-LOW");
        assert_eq!(issues[0].rule_id, "REQ_PRIORITY");
    }
}
