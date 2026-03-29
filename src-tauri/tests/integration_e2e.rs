// FICHIER : src-tauri/tests/integration_e2e.rs

#[path = "common/mod.rs"]
mod common;

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::collections::manager::CollectionsManager;
use raise::model_engine::arcadia;
use raise::model_engine::loader::ModelLoader;
use raise::model_engine::validators::{DynamicValidator, ModelValidator};
use raise::rules_engine::ast::{Expr, Rule};
use raise::utils::prelude::*;

#[async_test]
async fn test_full_stack_integration() {
    // =========================================================================
    // ÉTAPE 1 : Infrastructure (JSON-DB)
    // =========================================================================
    let env = setup_test_env(LlmMode::Disabled).await;

    let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);

    // Injection du mapping ontologique
    let sys_mgr = CollectionsManager::new(
        &env.sandbox.storage,
        &env.sandbox.config.system_domain,
        &env.sandbox.config.system_db,
    );

    let _ = sys_mgr
        .create_collection(
            "configs",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;
    sys_mgr
        .upsert_document(
            "configs",
            json_value!({
                "_id": "ref:configs:handle:ontological_mapping",
                "search_spaces": [
                    { "layer": env.db, "collection": "la" }
                ]
            }),
        )
        .await
        .unwrap();

    // =========================================================================
    // ÉTAPE 2 : Peuplement des Données
    // =========================================================================
    let valid_json = json_value!({
        "_id": "UUID_VALID_1",
        arcadia::PROP_ID: "UUID_VALID_1",
        arcadia::PROP_NAME: "ValidComponent",
        "@type": "LogicalComponent",
        arcadia::PROP_DESCRIPTION: "Un composant parfaitement documenté."
    });

    let invalid_json = json_value!({
        "_id": "UUID_INVALID_1",
        arcadia::PROP_ID: "UUID_INVALID_1",
        arcadia::PROP_NAME: "UndocumentedThing",
        "@type": "LogicalComponent",
        // 🎯 FIX : On déclare explicitement le champ à 'null' pour que
        // l'évaluateur de règles trouve la variable et déclenche la violation.
        arcadia::PROP_DESCRIPTION: null
    });

    manager
        .create_collection(
            "la",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();
    manager
        .insert_raw("la", &valid_json)
        .await
        .expect("Insert A failed");
    manager
        .insert_raw("la", &invalid_json)
        .await
        .expect("Insert B failed");

    // =========================================================================
    // ÉTAPE 3 : Définition des Règles
    // =========================================================================
    let rule_expr = Expr::Not(Box::new(Expr::Eq(vec![
        Expr::Var(arcadia::PROP_DESCRIPTION.to_string()),
        Expr::Val(JsonValue::Null),
    ])));

    let rule = Rule {
        id: "RULE_DOC_MANDATORY".to_string(),
        target: "LogicalComponent".to_string(),
        expr: rule_expr,
        description: Some("La description est obligatoire.".to_string()),
        severity: Some("Error".to_string()),
    };

    // =========================================================================
    // ÉTAPE 4 : Chargement & Validation
    // =========================================================================
    let loader = ModelLoader::new_with_manager(manager);

    let count = loader.index_project().await.expect("Indexation failed");
    assert_eq!(count, 2, "Le loader aurait dû trouver 2 éléments");

    let validator = DynamicValidator::new(vec![rule]);
    let issues = validator.validate_full(&loader).await;

    // =========================================================================
    // ÉTAPE 5 : Vérification des Résultats
    // =========================================================================
    assert_eq!(
        issues.len(),
        1,
        "Il devrait y avoir exactement 1 violation de règle"
    );
    assert_eq!(issues[0].element_id, "UUID_INVALID_1");

    println!("✅ Test E2E réussi : Le nouveau moteur dynamique fonctionne en flux complet !");
}
