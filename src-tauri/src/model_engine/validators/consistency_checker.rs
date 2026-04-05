// FICHIER : src-tauri/src/model_engine/validators/consistency_checker.rs

use super::{ModelValidator, Severity, ValidationIssue};
use crate::json_db::jsonld::vocabulary::VocabularyRegistry;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::utils::prelude::*;

#[derive(Default)]
pub struct ConsistencyChecker;

impl ConsistencyChecker {
    pub fn new() -> Self {
        Self
    }

    /// Vérifie la logique locale (ID, Nom, Domaine des propriétés)
    pub fn check_local_logic(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let name = element.name.as_str();

        // 1. Vérification de l'ID technique
        if element.id.trim().is_empty() {
            issues.push(ValidationIssue {
                severity: Severity::Error,
                rule_id: "SYS_001".to_string(),
                element_id: "unknown".to_string(),
                message: format!("L'élément '{}' n'a pas d'identifiant unique (UUID).", name),
            });
        }

        // 2. Vérification du nom par défaut
        if name.trim().is_empty() || name == "Sans nom" {
            issues.push(ValidationIssue {
                severity: Severity::Warning,
                rule_id: "SYS_002".to_string(),
                element_id: element.id.clone(),
                message: "L'élément n'a pas de nom descriptif.".to_string(),
            });
        }

        // 3. Validation sémantique du domaine (Ontologie)
        let registry = VocabularyRegistry::global();
        for prop_key in element.properties.keys() {
            if let Some(prop_def) = registry.get_property(prop_key) {
                if let Some(domain_iri) = &prop_def.domain {
                    if !registry.is_subtype_of(&element.kind, domain_iri) {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            rule_id: "SEM_001".to_string(),
                            element_id: element.id.clone(),
                            message: format!(
                                "Violation de domaine : '{}' ne peut pas s'appliquer à un '{}' (Attendu: {}).",
                                prop_def.label, element.kind, domain_iri
                            ),
                        });
                    }
                }
            }
        }

        issues
    }

    /// Vérifie la validité des relations (Range de l'ontologie)
    async fn check_relationships(
        &self,
        element: &ArcadiaElement,
        loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let registry = VocabularyRegistry::global();

        for (prop_key, prop_val) in &element.properties {
            if let Some(prop_def) = registry.get_property(prop_key) {
                if let Some(range_iri) = &prop_def.range {
                    let target_ids = match prop_val {
                        JsonValue::String(s) => vec![s.clone()],
                        JsonValue::Array(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect(),
                        _ => vec![],
                    };

                    for target_id in target_ids {
                        // On ne vérifie que les références qui ressemblent à des IDs ou URIs
                        if target_id.starts_with("http") || target_id.len() > 20 {
                            if let Ok(target_el) = loader.get_element(&target_id).await {
                                if !registry.is_subtype_of(&target_el.kind, range_iri) {
                                    issues.push(ValidationIssue {
                                        severity: Severity::Warning,
                                        rule_id: "SEM_002".to_string(),
                                        element_id: element.id.clone(),
                                        message: format!(
                                            "Relation invalide : La cible '{}' est de type '{}', attendu '{}' pour la propriété '{}'.",
                                            target_el.name.as_str(), target_el.kind, range_iri, prop_def.label
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        issues
    }
}

#[async_interface]
impl ModelValidator for ConsistencyChecker {
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        let mut issues = self.check_local_logic(element);

        if !element.properties.is_empty() {
            let rel_issues = self.check_relationships(element, loader).await;
            issues.extend(rel_issues);
        }

        issues
    }

    /// 🎯 SCAN UNIVERSEL : On parcourt dynamiquement tout le modèle chargé
    async fn validate_full(&self, loader: &ModelLoader<'_>) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        if let Ok(model) = loader.load_full_model().await {
            // 🚀 Utilisation de l'itérateur dynamique all_elements()
            for el in model.all_elements() {
                all_issues.extend(self.validate_element(el, loader).await);
            }
        }

        all_issues
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::NameType;
    use crate::utils::testing::AgentDbSandbox;

    async fn inject_mock_mapping(manager: &CollectionsManager<'_>) {
        let _ = manager
            .create_collection(
                "configs",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;
        manager
            .upsert_document(
                "configs",
                json_value!({
                    "_id": "ref:configs:handle:ontological_mapping",
                    "search_spaces": [ { "layer": "oa", "collection": "actors" } ]
                }),
            )
            .await
            .unwrap();
    }

    #[test]
    fn test_consistency_local_logic() {
        let checker = ConsistencyChecker::new();
        let el = ArcadiaElement {
            id: "UUID-OK".to_string(),
            name: NameType::String("ValidName".to_string()),
            kind: "https://raise.io/ontology/arcadia/la#LogicalComponent".to_string(),
            ..Default::default()
        };
        let issues = checker.check_local_logic(&el);
        assert!(issues.is_empty());
    }

    #[async_test]
    async fn test_consistency_full_scan_dynamic() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_mapping(&manager).await;

        let oa_mgr = CollectionsManager::new(&sandbox.db, &sandbox.config.system_domain, "oa");
        AgentDbSandbox::mock_db(&oa_mgr)
            .await
            .expect("Le setup de la DB 'la' a échoué");
        let _ = oa_mgr
            .create_collection(
                "actors",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;

        // Insertion d'un élément avec nom vide dans la couche OA
        oa_mgr
            .insert_raw(
                "actors",
                &json_value!({
                    "_id": "ACT-EMPTY", "name": "", "type": "OperationalActor"
                }),
            )
            .await
            .unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        let checker = ConsistencyChecker::new();

        let issues = checker.validate_full(&loader).await;

        // Vérification de la détection
        let found = issues
            .iter()
            .any(|i| i.element_id == "ACT-EMPTY" && i.rule_id == "SYS_002");
        assert!(
            found,
            "Le checker doit trouver l'erreur via le scan universel"
        );
    }
}
