// FICHIER : src-tauri/tests/rules_suite/rules_integration.rs

use raise::json_db::collections;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[tokio::test] // CORRECTION : Passage en test asynchrone pour supporter les appels .await
async fn test_end_to_end_rules_execution() {
    // 1. SETUP
    let dir = tempdir().unwrap();
    let config = JsonDbConfig::new(dir.path().to_path_buf());

    let space = "test_space";
    let db = "test_db";
    let storage = StorageEngine::new(config.clone());

    // CORRECTION E0599 : init_db() est désormais asynchrone, ajout de .await
    collections::manager::CollectionsManager::new(&storage, space, db)
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

    let schema_inv_path = config
        .db_schemas_root(space, db)
        .join("v1/invoices/default.json");

    fs::create_dir_all(schema_inv_path.parent().unwrap()).unwrap();
    fs::write(&schema_inv_path, schema_content.to_string()).unwrap();

    // 3. Création collection
    // CORRECTION E0599 : create_collection est désormais asynchrone
    collections::create_collection(&config, space, db, "invoices")
        .await
        .unwrap();

    // 4. EXECUTION
    let invoice_input = json!({
        "id": "inv_001",
        "user_id": "u_dev",
        "qty": 2,
        "price": 50
    });

    // CORRECTION E0599 : insert_with_schema est désormais asynchrone
    let result =
        collections::insert_with_schema(&config, space, db, "invoices/default.json", invoice_input)
            .await
            .expect("Insert invoice failed");

    // 5. VALIDATIONS
    assert_eq!(result["total"], 100.0);
    assert_eq!(result["ref"], "INV-U_DEV-100");
}
