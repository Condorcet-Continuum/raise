use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::model_engine::validators::{ModelValidator, Severity, ValidationIssue};

/// Validateur de conformité méthodologique.
/// Vérifie la qualité des données (Noms, Descriptions) et les règles de construction Arcadia.

#[derive(Default)]
pub struct ComplianceValidator;

impl ModelValidator for ComplianceValidator {
    fn validate(&self, model: &ProjectModel) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();

        // 1. Analyse de la couche System Analysis (SA)
        self.check_layer_quality("SA", &model.sa.components, &mut issues);
        self.check_layer_quality("SA", &model.sa.functions, &mut issues);

        // 2. Analyse de la couche Logical Architecture (LA)
        self.check_layer_quality("LA", &model.la.components, &mut issues);
        self.check_layer_quality("LA", &model.la.functions, &mut issues);

        // 3. Analyse de la couche Physical Architecture (PA)
        self.check_layer_quality("PA", &model.pa.components, &mut issues);
        self.check_layer_quality("PA", &model.pa.functions, &mut issues);

        // 4. Règles spécifiques Arcadia
        self.check_allocation_completeness(&model.la.components, "Logical Component", &mut issues);
        self.check_allocation_completeness(&model.pa.components, "Physical Component", &mut issues);

        issues
    }
}

impl ComplianceValidator {
    pub fn new() -> Self {
        Self
    }

    /// Vérifie la qualité de base (Nom, Description) pour une liste d'éléments
    fn check_layer_quality(
        &self,
        layer_name: &str,
        elements: &[ArcadiaElement],
        issues: &mut Vec<ValidationIssue>,
    ) {
        for el in elements {
            // Règle 1: Convention de nommage
            let name = el.name.as_str().trim();
            if name.is_empty() || name == "Unnamed" || name.starts_with("Copy of") {
                issues.push(ValidationIssue {
                    severity: Severity::Warning,
                    element_id: el.id.clone(),
                    message: format!(
                        "[{}] Élément mal nommé ('{}'). Utilisez un nom explicite.",
                        layer_name, name
                    ),
                    rule_id: "RULE_NAMING".to_string(),
                });
            }

            // Règle 2: Présence de description (Qualité documentaire)
            // On considère que tout élément majeur devrait être décrit
            let has_desc = el
                .description
                .as_ref()
                .map(|d| !d.is_empty())
                .unwrap_or(false);
            if !has_desc {
                issues.push(ValidationIssue {
                    severity: Severity::Info,
                    element_id: el.id.clone(),
                    message: format!("[{}] Description manquante pour '{}'", layer_name, name),
                    rule_id: "RULE_DOC_MISSING".to_string(),
                });
            }
        }
    }

    /// Vérifie qu'un composant structurel possède bien des fonctions allouées
    /// (Un composant sans comportement est souvent une erreur de modélisation)
    fn check_allocation_completeness(
        &self,
        components: &[ArcadiaElement],
        type_label: &str,
        issues: &mut Vec<ValidationIssue>,
    ) {
        for comp in components {
            // On ignore les composants "Actor" ou "System" génériques s'ils sont mal classés
            if comp.kind.contains("Actor") {
                continue;
            }

            // Récupération "Loose" de la propriété allocatedFunctions via la Map JSON
            // car nous sommes sur le modèle générique ici
            let has_functions = if let Some(val) = comp.properties.get("allocatedFunctions") {
                val.as_array().map(|arr| !arr.is_empty()).unwrap_or(false)
            } else {
                false
            };

            // Si aucune fonction et pas marqué comme "Abstrait" (approximation)
            if !has_functions {
                // On vérifie si c'est un noeud physique (qui peut ne pas avoir de fonction mais héberger des composants)
                let is_node = comp.kind.contains("Node");

                let severity = if is_node {
                    Severity::Info
                } else {
                    Severity::Warning
                };

                issues.push(ValidationIssue {
                    severity,
                    element_id: comp.id.clone(),
                    message: format!(
                        "{} '{}' ne possède aucune fonction allouée.",
                        type_label,
                        comp.name.as_str()
                    ),
                    rule_id: "RULE_EMPTY_COMPONENT".to_string(),
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::NameType;
    use std::collections::HashMap;

    #[test]
    fn test_naming_validation() {
        let mut model = ProjectModel::default();

        // Un composant mal nommé
        let bad_comp = ArcadiaElement {
            id: "bad_1".into(),
            name: NameType::String("Unnamed".into()),
            kind: "LogicalComponent".into(),
            description: None,
            properties: HashMap::new(),
        };

        // Un composant bien nommé
        let good_comp = ArcadiaElement {
            id: "good_1".into(),
            name: NameType::String("Engine Control".into()),
            kind: "LogicalComponent".into(),
            description: Some("Controls the engine".into()),
            properties: HashMap::new(),
        };

        model.la.components.push(bad_comp);
        model.la.components.push(good_comp);

        let validator = ComplianceValidator::new();
        let issues = validator.validate(&model);

        // On s'attend à :
        // 1 Warning pour "Unnamed"
        // 1 Info pour "Description manquante" sur bad_comp (si on l'a ajouté)
        // 1 Warning pour "Pas de fonction allouée" sur bad_comp
        // 1 Warning pour "Pas de fonction allouée" sur good_comp

        let naming_issues: Vec<_> = issues
            .iter()
            .filter(|i| i.rule_id == "RULE_NAMING")
            .collect();
        assert_eq!(naming_issues.len(), 1);
        assert_eq!(naming_issues[0].element_id, "bad_1");
    }
}
