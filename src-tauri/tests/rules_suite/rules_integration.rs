// FICHIER : src-tauri/tests/rules_suite/rules_integration.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::collections;
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*;
use std::fs;

#[tokio::test]
async fn test_end_to_end_rules_execution() {
    // 1. SETUP ROBUSTE
    let env = setup_test_env(LlmMode::Disabled).await;
    let config = &env.storage.config;

    // On utilise l'espace et la DB fournis par l'environnement
    let space = &env.space;
    let db = &env.db;

    // L'init_db est déjà fait par setup_test_env, mais on peut le rappeler par sécurité
    // (create_db est idempotent)
    CollectionsManager::new(&env.storage, space, db)
        .init_db()
        .await
        .unwrap();

    // 2. CRÉATION DU SCHÉMA
    let schema_content = json!({
        "type": "object",
        "properties": {
            "qty": { "type": "number" },
            "price": { "type": "number" },
            "total": { "type": "number" },
            "ref": { "type": "string" },
            "user_id": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "calc_total",
                "target": "total",
                "expr": { "mul": [ { "var": "qty" }, { "var": "price" } ] }
            },
            {
                "id": "gen_ref",
                "target": "ref",
                "expr": {
                    "concat": [
                        { "val": "INV-" },
                        { "upper": { "var": "user_id" } },
                        { "val": "-" },
                        { "var": "total" }
                    ]
                }
            }
        ]
    });

    // On écrit le schéma dans le dossier temporaire du test
    let schema_inv_path = config
        .db_schemas_root(space, db)
        .join("v1/invoices/default.json");

    fs::create_dir_all(schema_inv_path.parent().unwrap()).unwrap();
    fs::write(&schema_inv_path, schema_content.to_string()).unwrap();

    // 3. Création collection
    collections::create_collection(config, space, db, "invoices")
        .await
        .unwrap();

    // 4. EXECUTION
    let invoice_input = json!({
        "id": "inv_001",
        "user_id": "u_dev",
        "qty": 2,
        "price": 50
    });

    let result =
        collections::insert_with_schema(config, space, db, "invoices/default.json", invoice_input)
            .await
            .expect("Insert invoice failed");

    // 5. VALIDATIONS
    assert_eq!(result["total"], 100.0);
    // INV-U_DEV-100
    assert_eq!(result["ref"], "INV-U_DEV-100");
}
