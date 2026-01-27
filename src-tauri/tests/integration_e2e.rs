// FICHIER : src-tauri/tests/integration_e2e.rs

use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::model_engine::arcadia;
use raise::model_engine::loader::ModelLoader;
use raise::model_engine::validators::{DynamicValidator, ModelValidator, Severity};
use raise::rules_engine::ast::{Expr, Rule};
use serde_json::json;
use tempfile::tempdir;

#[tokio::test]
async fn test_full_stack_integration() {
    // =========================================================================
    // ÉTAPE 1 : Infrastructure (JSON-DB)
    // =========================================================================
    // On crée un environnement isolé (dossier temporaire)
    let dir = tempdir().unwrap();
    let config = JsonDbConfig::new(dir.path().to_path_buf());
    let storage = StorageEngine::new(config);

    // On initialise un projet "TestSpace/TestDB"
    let manager = CollectionsManager::new(&storage, "TestSpace", "TestDB");
    manager
        .init_db()
        .await
        .expect("Impossible d'initialiser la DB");

    // =========================================================================
    // ÉTAPE 2 : Peuplement des Données (Simulation de sauvegarde)
    // =========================================================================
    // On injecte deux éléments directement en JSON dans la collection 'la' (Logical Architecture)

    // Élément A : VALIDE (Possède une description)
    let valid_json = json!({
        arcadia::PROP_ID: "UUID_VALID_1",
        arcadia::PROP_NAME: "ValidComponent",
        "@type": "LogicalComponent", // Sera résolu en arcadia::KIND_LA_COMPONENT
        arcadia::PROP_DESCRIPTION: "Un composant parfaitement documenté."
    });

    // Élément B : INVALIDE (Pas de description)
    let invalid_json = json!({
        arcadia::PROP_ID: "UUID_INVALID_1",
        arcadia::PROP_NAME: "UndocumentedThing",
        "@type": "LogicalComponent"
        // Pas de description -> Doit déclencher la règle
    });

    manager
        .insert_raw("la", &valid_json)
        .await
        .expect("Insert A failed");
    manager
        .insert_raw("la", &invalid_json)
        .await
        .expect("Insert B failed");

    // =========================================================================
    // ÉTAPE 3 : Définition des Règles (Rules Engine)
    // =========================================================================
    // Règle : "Si un élément est un Composant Logique, il DOIT avoir une description non vide."

    // Expression AST : description != null AND description != ""
    // Simplification pour le test : description != null
    let rule_expr = Expr::Not(Box::new(Expr::Eq(vec![
        Expr::Var(arcadia::PROP_DESCRIPTION.to_string()),
        Expr::Val(serde_json::Value::Null),
    ])));

    let rule = Rule {
        id: "RULE_DOC_MANDATORY".to_string(),
        target: "la.components".to_string(), // Cible les composants logiques
        expr: rule_expr,
        description: Some("La description est obligatoire.".to_string()),
        severity: Some("Error".to_string()),
    };

    // =========================================================================
    // ÉTAPE 4 : Chargement & Validation (Model Engine)
    // =========================================================================

    // 1. Instanciation du Loader (qui va lire la DB temporaire)
    let loader = ModelLoader::new_with_manager(manager);

    // 2. Indexation (Découverte des fichiers créés)
    let count = loader.index_project().await.expect("Indexation failed");
    assert_eq!(count, 2, "Le loader aurait dû trouver 2 éléments");

    // 3. Exécution du Validateur Dynamique
    let validator = DynamicValidator::new(vec![rule]);

    // C'est ici que tout s'articule :
    // Loader -> Charge les données
    // Validator -> Parcourt le modèle
    // Rules Engine -> Évalue la règle sur chaque élément
    let issues = validator.validate_full(&loader).await;

    // =========================================================================
    // ÉTAPE 5 : Vérification des Résultats
    // =========================================================================

    // On s'attend à 1 seule erreur (sur l'élément invalide)
    assert_eq!(
        issues.len(),
        1,
        "Il devrait y avoir exactement 1 violation de règle"
    );

    let issue = &issues[0];
    assert_eq!(issue.rule_id, "RULE_DOC_MANDATORY");
    assert_eq!(issue.element_id, "UUID_INVALID_1");
    assert_eq!(issue.severity, Severity::Error);

    println!("✅ Test E2E réussi : Le système a détecté l'élément non documenté !");
}
