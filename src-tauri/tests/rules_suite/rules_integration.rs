use raise::json_db::collections;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use serde_json::json;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_end_to_end_rules_execution() {
    // 1. SETUP : Environnement temporaire
    let dir = tempdir().unwrap();

    // CORRECTION ICI : 'data_root' au lieu de 'root_dir'
    let config = JsonDbConfig {
        data_root: dir.path().to_path_buf(),
    };

    let space = "test_space";
    let db = "test_db";
    let storage = StorageEngine::new(config.clone());

    // Initialiser la DB
    collections::manager::CollectionsManager::new(&storage, space, db)
        .init_db()
        .unwrap();

    // 2. CRÉATION DU SCHÉMA avec x_rules
    let schema_content = json!({
        "type": "object",
        "properties": {
            "qty": { "type": "number" },
            "price": { "type": "number" },
            "total": { "type": "number" },
            "category": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "calc_total",
                "target": "total",
                "expr": { "mul": [ { "var": "qty" }, { "var": "price" } ] }
            },
            {
                "id": "calc_category",
                "target": "category",
                "expr": {
                    "if": {
                        "condition": { "gte": [ { "var": "total" }, { "val": 100.0 } ] },
                        "then_branch": { "val": "Premium" },
                        "else_branch": { "val": "Standard" }
                    }
                }
            }
        ]
    });

    let schema_path = config.db_schemas_root(space, db).join("v1/orders.json");
    fs::create_dir_all(schema_path.parent().unwrap()).unwrap();
    fs::write(&schema_path, schema_content.to_string()).unwrap();

    // 3. INSERTION
    let doc_input = json!({
        "id": "ord_001",
        "qty": 10,
        "price": 15.0
    });

    let result = collections::insert_with_schema(&config, space, db, "orders.json", doc_input)
        .expect("Insertion échouée");

    // 4. VÉRIFICATIONS
    println!(
        "Résultat : {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    assert_eq!(result["total"], 150.0);
    assert_eq!(result["category"], "Premium");
}

#[test]
fn test_advanced_business_rules() {
    // 1. SETUP
    let dir = tempdir().unwrap();
    let config = JsonDbConfig {
        data_root: dir.path().to_path_buf(),
    };
    let space = "biz_space";
    let db = "biz_db";
    let storage = StorageEngine::new(config.clone());
    let mgr = collections::manager::CollectionsManager::new(&storage, space, db);
    mgr.init_db().unwrap();

    // 2. DATA : Création du schéma et de l'utilisateur

    // A. Schéma Users : On le place dans "users/default.json" pour que la collection soit "users"
    let schema_users = json!({
        "type": "object",
        "properties": {
            "id": { "type": "string" },
            "name": { "type": "string" },
            "tjm": { "type": "number" }
        }
    });

    // Attention au chemin : v1/users/default.json
    let schema_users_path = config
        .db_schemas_root(space, db)
        .join("v1/users/default.json");
    fs::create_dir_all(schema_users_path.parent().unwrap()).unwrap();
    fs::write(&schema_users_path, schema_users.to_string()).unwrap();

    // B. Création de la collection "users"
    collections::create_collection(&config, space, db, "users").unwrap();

    let user_doc = json!({ "id": "u_dev", "name": "Alice", "tjm": 500.0 });

    // Insertion avec le bon chemin relatif
    collections::insert_with_schema(&config, space, db, "users/default.json", user_doc).unwrap();

    // 3. SCHÉMA FACTURE (Invoices)
    // On le place dans "invoices/default.json"
    let schema_invoice = json!({
        "type": "object",
        "properties": {
            "user_id": { "type": "string" },
            "days": { "type": "number" },
            "created_at": { "type": "string" },
            "total": { "type": "number" },
            "due_at": { "type": "string" },
            "ref": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "calc_total_from_lookup",
                "target": "total",
                "expr": {
                    "mul": [
                        { "var": "days" },
                        { "lookup": { "collection": "users", "id": { "var": "user_id" }, "field": "tjm" } }
                    ]
                }
            },
            {
                "id": "calc_due_date",
                "target": "due_at",
                "expr": { "date_add": { "date": { "var": "created_at" }, "days": { "val": 30 } } }
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

    // Chemin : v1/invoices/default.json
    let schema_inv_path = config
        .db_schemas_root(space, db)
        .join("v1/invoices/default.json");
    fs::create_dir_all(schema_inv_path.parent().unwrap()).unwrap();
    fs::write(&schema_inv_path, schema_invoice.to_string()).unwrap();

    // Création de la collection "invoices"
    collections::create_collection(&config, space, db, "invoices").unwrap();

    // 4. EXECUTION : Création de la facture
    let invoice_input = json!({
        "id": "inv_001",
        "user_id": "u_dev",
        "days": 10,
        "created_at": "2025-01-01T00:00:00Z"
    });

    let result =
        collections::insert_with_schema(&config, space, db, "invoices/default.json", invoice_input)
            .expect("Insert invoice");
    println!(
        "Facture générée : {}",
        serde_json::to_string_pretty(&result).unwrap()
    );

    // 5. VALIDATIONS
    assert_eq!(result["total"], 5000.0, "Le Lookup TJM a échoué");
    assert_eq!(
        result["due_at"], "2025-01-31T00:00:00+00:00",
        "Le calcul de date a échoué"
    );
    assert_eq!(result["ref"], "INV-U_DEV-5000", "La concaténation a échoué");
}
