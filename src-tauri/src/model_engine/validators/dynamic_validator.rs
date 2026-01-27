// FICHIER : src-tauri/src/model_engine/validators/dynamic_validator.rs

use crate::model_engine::arcadia; // <-- Import Vocabulaire
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::model_engine::validators::{ModelValidator, Severity, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::Evaluator;
use async_trait::async_trait;
use serde_json::json;
use serde_json::Value;

pub struct DynamicValidator {
    rules: Vec<Rule>,
}

impl DynamicValidator {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Vérifie si une règle s'applique à un élément donné via son type (Kind).
    fn rule_applies_to(&self, rule_target: &str, element_kind: &str) -> bool {
        // Normalisation : On utilise 'contains' pour supporter les URIs complètes et les noms courts
        // Ex: "https://.../LogicalComponent" ou "LogicalComponent"
        match rule_target {
            // OA
            "oa.actors" => {
                element_kind == arcadia::KIND_OA_ACTOR || element_kind.contains("OperationalActor")
            }
            "oa.activities" => {
                element_kind == arcadia::KIND_OA_ACTIVITY
                    || element_kind.contains("OperationalActivity")
            }
            "oa.capabilities" => {
                element_kind == arcadia::KIND_OA_CAPABILITY
                    || element_kind.contains("OperationalCapability")
            }
            "oa.entities" => {
                element_kind == arcadia::KIND_OA_ENTITY
                    || element_kind.contains("OperationalEntity")
            }

            // SA
            "sa.components" => {
                element_kind == arcadia::KIND_SA_COMPONENT
                    || element_kind.contains("SystemComponent")
            }
            "sa.functions" => {
                element_kind == arcadia::KIND_SA_FUNCTION || element_kind.contains("SystemFunction")
            }
            "sa.actors" => {
                element_kind == arcadia::KIND_SA_ACTOR || element_kind.contains("SystemActor")
            }

            // LA
            "la.components" => {
                element_kind == arcadia::KIND_LA_COMPONENT
                    || element_kind.contains("LogicalComponent")
            }
            "la.functions" => {
                element_kind == arcadia::KIND_LA_FUNCTION
                    || element_kind.contains("LogicalFunction")
            }

            // PA
            "pa.components" => {
                element_kind == arcadia::KIND_PA_COMPONENT
                    || element_kind.contains("PhysicalComponent")
            }

            // Generic
            "all" => true,
            _ => false, // Cible inconnue ou non mappée
        }
    }
}

#[async_trait]
impl ModelValidator for DynamicValidator {
    // Validation Unitaire (Temps Réel)
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // Conversion de l'élément en JSON contextuel pour l'évaluateur
        let mut context = json!({
            "id": element.id,
            "name": element.name.as_str(),
            "kind": element.kind,
            "description": element.description
        });

        // Fusion des propriétés dynamiques
        if let Some(obj) = context.as_object_mut() {
            for (k, v) in &element.properties {
                obj.insert(k.clone(), v.clone());
            }
        }

        for rule in &self.rules {
            if self.rule_applies_to(&rule.target, &element.kind) {
                // On passe 'loader' comme DataProvider pour permettre les Lookups
                match Evaluator::evaluate(&rule.expr, &context, loader).await {
                    Ok(result) => {
                        let is_valid = match result.as_ref() {
                            Value::Bool(b) => *b,
                            Value::Null => false,
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
                                message: rule.description.clone().unwrap_or_else(|| {
                                    format!("Règle '{}' non respectée", rule.id)
                                }),
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
        }

        issues
    }

    // Validation Globale (Rapport)
    async fn validate_full(&self, loader: &ModelLoader<'_>) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        if let Ok(model) = loader.load_full_model().await {
            // Helper asynchrone pour traiter une liste
            async fn check_list(
                validator: &DynamicValidator,
                elements: &[ArcadiaElement],
                loader: &ModelLoader<'_>,
                issues: &mut Vec<ValidationIssue>,
            ) {
                for el in elements {
                    let res = validator.validate_element(el, loader).await;
                    issues.extend(res);
                }
            }

            // OA
            check_list(self, &model.oa.actors, loader, &mut all_issues).await;
            check_list(self, &model.oa.activities, loader, &mut all_issues).await;

            // SA
            check_list(self, &model.sa.components, loader, &mut all_issues).await;
            check_list(self, &model.sa.functions, loader, &mut all_issues).await;

            // LA
            check_list(self, &model.la.components, loader, &mut all_issues).await;
            check_list(self, &model.la.functions, loader, &mut all_issues).await;

            // PA
            check_list(self, &model.pa.components, loader, &mut all_issues).await;
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
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn setup_loader() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[tokio::test]
    async fn test_dynamic_validator_integration() {
        let (_dir, config) = setup_loader();
        let storage = StorageEngine::new(config);
        let loader = ModelLoader::new_with_manager(
            crate::json_db::collections::manager::CollectionsManager::new(&storage, "t", "d"),
        );

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

        // Utilisation constante
        let valid_el = ArcadiaElement {
            id: "1".into(),
            name: NameType::String("ValidElement".into()),
            kind: arcadia::KIND_OA_ACTOR.into(),
            description: None,
            properties: HashMap::new(),
        };

        let invalid_el = ArcadiaElement {
            id: "2".into(),
            name: NameType::String("BadName".into()),
            kind: arcadia::KIND_OA_ACTOR.into(),
            description: None,
            properties: HashMap::new(),
        };

        let issues_1 = validator.validate_element(&valid_el, &loader).await;
        assert!(issues_1.is_empty());

        let issues_2 = validator.validate_element(&invalid_el, &loader).await;
        assert_eq!(issues_2.len(), 1);
        assert_eq!(issues_2[0].rule_id, "TEST_RULE");
    }
}
