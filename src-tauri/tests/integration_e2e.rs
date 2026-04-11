// FICHIER : src-tauri/tests/integration_e2e.rs

#[path = "common/mod.rs"]
mod common;

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::collections::manager::CollectionsManager;
use raise::model_engine::arcadia;
use raise::model_engine::loader::ModelLoader;
use raise::model_engine::validators::{DynamicValidator, ModelValidator};
use raise::rules_engine::ast::{Expr, Rule};
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

#[async_test]
async fn test_full_stack_integration() -> RaiseResult<()> {
    // =========================================================================
    // ÉTAPE 1 : Infrastructure (JSON-DB & Mount Points)
    // =========================================================================
    let env = setup_test_env(LlmMode::Disabled).await;

    // 🎯 RÉSILIENCE : Utilisation des partitions injectées par la Sandbox
    let manager = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);

    // Résolution dynamique du point de montage système
    let system_domain = &env.sandbox.config.mount_points.system.domain;
    let system_db = &env.sandbox.config.mount_points.system.db;

    let sys_mgr = CollectionsManager::new(&env.sandbox.storage, system_domain, system_db);

    let generic_schema = "db://_system/_system/schemas/v1/db/generic.schema.json";

    match sys_mgr.create_collection("configs", generic_schema).await {
        Ok(_) => user_info!("INF_TEST_CONFIG_COLL_READY"),
        Err(e) => raise_error!("ERR_TEST_SETUP_FAIL", error = e.to_string()),
    }

    // Injection du mapping ontologique pour le Loader
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
        .await?;

    // =========================================================================
    // ÉTAPE 2 : Peuplement des Données (MBSE Arcadia)
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
        arcadia::PROP_DESCRIPTION: null
    });

    manager.create_collection("la", generic_schema).await?;

    match manager.insert_raw("la", &valid_json).await {
        Ok(_) => (),
        Err(e) => raise_error!("ERR_TEST_DATA_INJECTION", error = e.to_string()),
    }

    match manager.insert_raw("la", &invalid_json).await {
        Ok(_) => (),
        Err(e) => raise_error!("ERR_TEST_DATA_INJECTION", error = e.to_string()),
    }

    // =========================================================================
    // ÉTAPE 3 : Définition des Règles (AST)
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

    match loader.index_project().await {
        Ok(count) => {
            assert_eq!(count, 2, "Le loader aurait dû trouver 2 éléments");
            user_success!("SUC_TEST_INDEXATION_OK", json_value!({"count": count}));
        }
        Err(e) => raise_error!("ERR_TEST_INDEXATION_FAIL", error = e.to_string()),
    }

    let validator = DynamicValidator::new(vec![rule]);
    let issues = validator.validate_full(&loader).await;

    // =========================================================================
    // ÉTAPE 5 : Vérification des Résultats
    // =========================================================================
    assert_eq!(issues.len(), 1, "Il devrait y avoir exactement 1 violation");
    assert_eq!(issues[0].element_id, "UUID_INVALID_1");

    user_success!("SUC_TEST_E2E_FULL_STACK_VALIDATED");
    Ok(())
}

// =========================================================================
// NOUVEAUX TESTS : RÉSILIENCE ET POINTS DE MONTAGE
// =========================================================================

#[cfg(test)]
mod resilience_tests {
    use super::*;

    /// 🎯 Test la résilience face à la résolution des partitions via Mount Points
    #[async_test]
    async fn test_e2e_mount_point_integrity() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;
        // Validation SSOT de la partition système injectée dans la sandbox
        assert!(!env.sandbox.config.mount_points.system.domain.is_empty());
        assert!(!env.sandbox.config.mount_points.system.db.is_empty());
        Ok(())
    }

    /// 🎯 Test la résilience du loader en cas de partition manquante (Match...raise_error)
    #[async_test]
    async fn test_e2e_loader_missing_partition_resilience() -> RaiseResult<()> {
        let env = setup_test_env(LlmMode::Disabled).await;

        // On initialise un loader sur une partition qui n'existe pas physiquement
        let ghost_mgr = CollectionsManager::new(&env.sandbox.storage, "ghost_space", "ghost_db");
        let loader = ModelLoader::new_with_manager(ghost_mgr);

        match loader.index_project().await {
            Ok(count) => {
                // Si la partition n'existe pas, l'indexation doit retourner 0 sans paniquer
                assert_eq!(count, 0);
                Ok(())
            }
            Err(e) => raise_error!("ERR_TEST_LOADER_CRASH", error = e.to_string()),
        }
    }
}
