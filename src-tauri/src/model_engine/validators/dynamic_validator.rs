// FICHIER : src-tauri/src/model_engine/validators/dynamic_validator.rs

use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::model_engine::validators::{ModelValidator, Severity, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use serde_json::Value;

/// Validateur capable d'appliquer des règles dynamiques (AST) définies dans des fichiers JSON.
pub struct DynamicValidator {
    rules: Vec<Rule>,
}

impl DynamicValidator {
    pub fn new(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Mappe une chaîne de caractères (ex: "sa.components") vers la collection réelle du modèle.
    fn resolve_target<'a>(
        &self,
        target: &str,
        model: &'a ProjectModel,
    ) -> Option<&'a Vec<ArcadiaElement>> {
        match target {
            // Operational Analysis
            "oa.actors" => Some(&model.oa.actors),
            "oa.activities" => Some(&model.oa.activities),
            "oa.capabilities" => Some(&model.oa.capabilities),
            "oa.entities" => Some(&model.oa.entities),
            "oa.exchanges" => Some(&model.oa.exchanges),

            // System Analysis
            "sa.components" => Some(&model.sa.components),
            "sa.actors" => Some(&model.sa.actors),
            "sa.functions" => Some(&model.sa.functions),
            "sa.capabilities" => Some(&model.sa.capabilities),
            "sa.exchanges" => Some(&model.sa.exchanges),

            // Logical Architecture
            "la.components" => Some(&model.la.components),
            "la.actors" => Some(&model.la.actors),
            "la.functions" => Some(&model.la.functions),
            "la.interfaces" => Some(&model.la.interfaces),
            "la.exchanges" => Some(&model.la.exchanges),

            // Physical Architecture
            "pa.components" => Some(&model.pa.components),
            "pa.actors" => Some(&model.pa.actors),
            "pa.functions" => Some(&model.pa.functions),
            "pa.links" => Some(&model.pa.links),
            "pa.exchanges" => Some(&model.pa.exchanges),

            // EPBS
            "epbs.configuration_items" => Some(&model.epbs.configuration_items),

            // Data
            "data.classes" => Some(&model.data.classes),
            "data.data_types" => Some(&model.data.data_types),
            "data.exchange_items" => Some(&model.data.exchange_items),

            _ => None,
        }
    }
}

impl ModelValidator for DynamicValidator {
    fn validate(&self, model: &ProjectModel) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        // Le provider NoOp est suffisant car nous injectons l'élément complet comme contexte
        let provider = NoOpDataProvider;

        for rule in &self.rules {
            if let Some(elements) = self.resolve_target(&rule.target, model) {
                for element in elements {
                    // 1. Conversion de l'élément Arcadia en JSON pour l'évaluateur
                    let context_value = match serde_json::to_value(element) {
                        Ok(v) => v,
                        Err(e) => {
                            eprintln!(
                                "Erreur de sérialisation pour l'élément {}: {}",
                                element.id, e
                            );
                            continue;
                        }
                    };

                    // 2. Évaluation de la règle
                    // Convention : L'expression AST décrit la condition de VALIDITÉ.
                    // Si result == false => Violation.
                    match Evaluator::evaluate(&rule.expr, &context_value, &provider) {
                        Ok(result_cow) => {
                            let is_valid = match result_cow.as_ref() {
                                Value::Bool(b) => *b,
                                Value::Null => false, // Null est considéré comme échec (ex: champ manquant)
                                _ => true, // Les autres valeurs sont considérées "truthy"
                            };

                            if !is_valid {
                                issues.push(ValidationIssue {
                                    severity: Severity::Warning, // Par défaut Warning
                                    rule_id: rule.id.clone(),
                                    element_id: element.id.clone(),
                                    message: format!(
                                        "Règle non respectée : {} (Cible: {})",
                                        rule.id, rule.target
                                    ),
                                });
                            }
                        }
                        Err(e) => {
                            eprintln!(
                                "Erreur d'évaluation règle {} sur élément {}: {}",
                                rule.id, element.id, e
                            );
                        }
                    }
                }
            }
        }

        issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::rules_engine::ast::Expr;
    use serde_json::json;

    // Helper pour créer un élément mocké
    fn create_element(name: &str, properties: Option<serde_json::Value>) -> ArcadiaElement {
        let mut el = ArcadiaElement::default();
        el.name = NameType::String(name.to_string());
        if let Some(props) = properties {
            if let Some(obj) = props.as_object() {
                el.properties = obj.clone().into_iter().collect();
            }
        }
        el
    }

    #[test]
    fn test_dynamic_validator_naming_regex() {
        let rule_expr = Expr::RegexMatch {
            value: Box::new(Expr::Var("name".to_string())),
            pattern: Box::new(Expr::Val(json!("^SA_"))),
        };

        let rule = Rule {
            id: "CHECK_PREFIX".to_string(),
            target: "sa.components".to_string(),
            expr: rule_expr,
            // CORRECTION : Ajout des champs manquants
            description: None,
            severity: None,
        };

        let validator = DynamicValidator::new(vec![rule]);
        let mut model = ProjectModel::default();

        // Création des éléments avec 2 arguments (nom, propriétés)
        model.sa.components.push(create_element("SA_System", None));
        model.sa.components.push(create_element("Bad_System", None));

        let issues = validator.validate(&model);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].element_id, model.sa.components[1].id);
    }
}
