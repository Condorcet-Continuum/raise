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
    let config = &env.sandbox.storage.config;

    // ✅ NOUVEAU : On crée une référence directe au StorageEngine
    let storage = &env.sandbox.storage;

    // On utilise l'espace et la DB fournis par l'environnement
    let space = &env.space;
    let db = &env.db;

    // L'init_db est déjà fait par setup_test_env, mais on peut le rappeler par sécurité
    // (create_db est idempotent)
    CollectionsManager::new(storage, space, db)
        .init_db()
        .await
        .unwrap();
    let manager = CollectionsManager::new(storage, space, db);
    manager.init_db().await.unwrap();

    // 🎯 FIX STRICT SCHEMA : On déclare légalement la collection _system_rules
    manager
        .create_collection(
            "_system_rules",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
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

    // On écrit le schéma dans le dossier temporaire du test (le config reste utile ici)
    let schema_inv_path = config
        .db_schemas_root(space, db)
        .join("v1/invoices/default.json");

    fs::create_dir_all(schema_inv_path.parent().unwrap()).unwrap();
    fs::write(&schema_inv_path, schema_content.to_string()).unwrap();

    // 3. Création collection
    // ✅ CORRECTION : Remplacement de `config` par `storage`
    collections::create_collection(storage, space, db, "invoices")
        .await
        .unwrap();

    // 4. EXECUTION
    let invoice_input = json!({
        "_id": "inv_001",
        "user_id": "u_dev",
        "qty": 2,
        "price": 50
    });

    // ✅ CORRECTION : Remplacement de `config` par `storage`
    let result =
        collections::insert_with_schema(storage, space, db, "invoices/default.json", invoice_input)
            .await
            .expect("Insert invoice failed");

    // 5. VALIDATIONS
    assert_eq!(result["total"], 100.0);
    // INV-U_DEV-100
    assert_eq!(result["ref"], "INV-U_DEV-100");
}
