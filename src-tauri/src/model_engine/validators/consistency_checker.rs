// FICHIER : src-tauri/src/model_engine/validators/consistency_checker.rs

use super::{ModelValidator, Severity, ValidationIssue};
use crate::json_db::jsonld::vocabulary::VocabularyRegistry; // Accès à l'ontologie
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ArcadiaElement;
use crate::utils::{async_trait, prelude::*};

/// Validateur de cohérence technique et sémantique.
#[derive(Default)]
pub struct ConsistencyChecker;

impl ConsistencyChecker {
    pub fn new() -> Self {
        Self
    }

    /// Logique de validation unitaire pure (sans accès base de données)
    /// Vérifie les champs obligatoires et les contraintes de domaine locales.
    pub fn check_local_logic(&self, element: &ArcadiaElement) -> Vec<ValidationIssue> {
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
                message: "L'élément n'a pas de nom descriptif.".to_string(),
            });
        }

        // RÈGLE 3 : Validation de Domaine Sémantique
        // On vérifie que les propriétés présentes sont autorisées pour ce type d'élément.
        let registry = VocabularyRegistry::global();

        for prop_key in element.properties.keys() {
            // Si la propriété est définie dans l'ontologie
            if let Some(prop_def) = registry.get_property(prop_key) {
                // Et si elle a un domaine restreint
                if let Some(domain_iri) = &prop_def.domain {
                    // On vérifie si le type de notre élément est bien un sous-type du domaine
                    if !registry.is_subtype_of(&element.kind, domain_iri) {
                        issues.push(ValidationIssue {
                            severity: Severity::Error,
                            rule_id: "SEM_001".to_string(),
                            element_id: element.id.clone(),
                            message: format!(
                                "Propriété invalide : '{}' ne peut pas s'appliquer à un '{}' (Attendu: {}).",
                                prop_def.label, element.kind, domain_iri
                            ),
                        });
                    }
                }
            }
        }

        issues
    }

    /// Logique de validation relationnelle (avec accès base de données)
    /// Vérifie les cibles des relations (Range check).
    async fn check_relationships(
        &self,
        element: &ArcadiaElement,
        loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        let mut issues = Vec::new();
        let registry = VocabularyRegistry::global();

        // Ici on garde l'itération complète car on a besoin de prop_val
        for (prop_key, prop_val) in &element.properties {
            if let Some(prop_def) = registry.get_property(prop_key) {
                // Si la propriété a un Range défini (type attendu de la cible)
                if let Some(range_iri) = &prop_def.range {
                    // Récupération des IDs cibles (tableau ou valeur simple)
                    let target_ids = match prop_val {
                        Value::String(s) => vec![s.clone()],
                        Value::Array(arr) => arr
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_string()))
                            .collect(),
                        _ => vec![],
                    };

                    for target_id in target_ids {
                        // On ignore les cibles qui ne sont pas des IRIs ou UUIDs valides pour le moment
                        if target_id.starts_with("http") || target_id.len() > 20 {
                            // Fetch de la cible pour vérifier son type
                            if let Ok(target_el) = loader.get_element(&target_id).await {
                                if !registry.is_subtype_of(&target_el.kind, range_iri) {
                                    issues.push(ValidationIssue {
                                        severity: Severity::Warning,
                                        rule_id: "SEM_002".to_string(),
                                        element_id: element.id.clone(),
                                        message: format!(
                                            "Relation invalide : La cible '{}' (via {}) est de type '{}', attendu '{}'.",
                                            target_el.name.as_str(), prop_def.label, target_el.kind, range_iri
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

    /// Helper pour valider une liste d'éléments (évite la duplication dans validate_full)
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
impl ModelValidator for ConsistencyChecker {
    async fn validate_element(
        &self,
        element: &ArcadiaElement,
        loader: &ModelLoader<'_>,
    ) -> Vec<ValidationIssue> {
        // 1. Contrôles locaux (rapides)
        let mut issues = self.check_local_logic(element);

        // 2. Contrôles relationnels (asynchrones, nécessitent des lectures DB)
        // Pour éviter de spammer la DB, on ne le fait que si l'élément semble avoir des relations sémantiques
        if !element.properties.is_empty() {
            let rel_issues = self.check_relationships(element, loader).await;
            issues.extend(rel_issues);
        }

        issues
    }

    // AJOUT : Implémentation explicite de la validation complète incluant la couche Transverse
    async fn validate_full(&self, loader: &ModelLoader<'_>) -> Vec<ValidationIssue> {
        let mut all_issues = Vec::new();

        if let Ok(model) = loader.load_full_model().await {
            // --- COUCHES CLASSIQUES ---
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

            // --- AJOUT : COUCHE TRANSVERSE ---
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
    use crate::json_db::jsonld::vocabulary::{arcadia_types, namespaces};
    use crate::model_engine::types::NameType;
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::{data::HashMap, io::tempdir};

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
        // Utilisation d'une URI valide pour passer la validation de domaine implicite
        let kind = format!("{}{}", namespaces::LA, arcadia_types::LA_COMPONENT);
        let el = create_dummy_element("UUID-1", "MyComponent", &kind);
        let issues = checker.check_local_logic(&el);
        assert!(issues.is_empty());
    }

    #[test]
    fn test_missing_name_warning() {
        let checker = ConsistencyChecker::new();
        let kind = format!("{}{}", namespaces::LA, arcadia_types::LA_COMPONENT);
        let el = create_dummy_element("UUID-2", "", &kind);
        let issues = checker.check_local_logic(&el);

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].severity, Severity::Warning);
        assert_eq!(issues[0].rule_id, "SYS_002");
    }

    // TEST SÉMANTIQUE : Domaine invalide
    #[test]
    fn test_domain_violation() {
        let checker = ConsistencyChecker::new();

        // Un PhysicalComponent...
        let kind_pa = format!("{}{}", namespaces::PA, arcadia_types::PA_COMPONENT);
        let mut el = create_dummy_element("UUID-BAD", "BadComponent", &kind_pa);

        // ... qui essaie d'avoir une propriété "involvesActivity" (réservée à OA Capability)
        let prop_iri = format!("{}involvesActivity", namespaces::OA);
        el.properties.insert(prop_iri, json!(["UUID-ACT-1"]));

        let issues = checker.check_local_logic(&el);

        assert!(
            !issues.is_empty(),
            "Devrait détecter une violation de domaine"
        );
        let err = &issues[0];
        assert_eq!(err.rule_id, "SEM_001");
        assert!(err.message.contains("ne peut pas s'appliquer"));
    }

    #[tokio::test]
    async fn test_full_validation_scans_transverse() {
        // SETUP : Création d'un environnement DB avec une Exigence mal formée
        use crate::json_db::collections::manager::CollectionsManager;
        use crate::json_db::storage::{JsonDbConfig, StorageEngine};
        inject_mock_config();

        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test_check", "db_check");
        manager.init_db().await.unwrap();

        // Insertion d'une Exigence SANS NOM (doit déclencher SYS_002)
        let invalid_req = json!({
            "id": "REQ-BAD",
            "name": "", // Nom vide -> Erreur
            "type": "https://raise.io/ontology/arcadia/transverse#Requirement"
        });
        manager
            .insert_raw("transverse", &invalid_req)
            .await
            .unwrap();

        let loader = ModelLoader::new_with_manager(manager);
        let checker = ConsistencyChecker::new();

        // EXECUTION : Validation globale
        let issues = checker.validate_full(&loader).await;

        // VERIFICATION
        // On s'attend à trouver une erreur SYS_002 sur REQ-BAD
        let found = issues
            .iter()
            .any(|i| i.element_id == "REQ-BAD" && i.rule_id == "SYS_002");
        assert!(
            found,
            "La validation globale n'a pas détecté l'exigence invalide dans la couche transverse."
        );
    }
}
