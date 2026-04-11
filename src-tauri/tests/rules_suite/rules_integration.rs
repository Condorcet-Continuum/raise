// FICHIER : src-tauri/tests/rules_suite/rules_integration.rs
use raise::utils::prelude::*;

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::collections::manager::CollectionsManager;

#[async_test]
async fn test_end_to_end_rules_execution() {
    // 1. SETUP ROBUSTE
    let env = setup_test_env(LlmMode::Disabled).await;
    let storage = &env.sandbox.storage;
    let space = &env.space;
    let db = &env.db;

    let manager = CollectionsManager::new(storage, space, db);
    // manager.init_db() est déjà appelé dans setup_test_env()

    // 🎯 FIX STRICT SCHEMA : On déclare légalement la collection _system_rules
    manager
        .create_collection(
            "_system_rules",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await
        .unwrap();

    // 2. CRÉATION DU SCHÉMA
    let schema_content = json_value!({
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

    // ✅ CORRECTION "ZÉRO DETTE" : On passe par le manager pour créer le schéma proprement
    manager
        .create_schema_def("v1/invoices/default.json", schema_content)
        .await
        .unwrap();

    // 3. CRÉATION DE LA COLLECTION via le Manager
    manager
        .create_collection("invoices", "v1/invoices/default.json")
        .await
        .unwrap();

    // 4. EXECUTION
    let invoice_input = json_value!({
        "_id": "inv_001",
        "user_id": "u_dev",
        "qty": 2,
        "price": 50
    });

    // ✅ CORRECTION : Utilisation de insert_with_schema du Manager (Gère les _id, les $schema et l'AST)
    let result = manager
        .insert_with_schema("invoices", invoice_input)
        .await
        .expect("Insert invoice failed");

    // 5. VALIDATIONS
    assert_eq!(result["total"], 100.0);
    assert_eq!(result["ref"], "INV-U_DEV-100");
}
