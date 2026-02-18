// FICHIER : src-tauri/tests/json_db_suite/dataset_integration.rs

use crate::common::{seed_mock_datasets, setup_test_env}; // Notre nouveau socle !
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::io::{self};
use raise::utils::prelude::*; // SSOT : Apporte Value, json, Result, etc.

#[tokio::test]
async fn debug_import_exchange_item() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);

    // 2. Création du fichier factice (remplace l'ancienne méthode)
    let data_path = seed_mock_datasets(&env.domain_path).await.unwrap();

    // 3. Lecture du fichier via notre SSOT
    let mut json_doc: Value = io::read_json(&data_path)
        .await
        .expect("Lecture ou parsing JSON impossible");

    // 4. Injection du schéma
    let schema_rel_path = "arcadia/data/exchange-item.schema.json";
    let db_schema_uri = format!(
        "db://{}/{}/schemas/v1/{}",
        env.space, env.db, schema_rel_path
    );

    if let Some(obj) = json_doc.as_object_mut() {
        obj.insert("$schema".to_string(), Value::String(db_schema_uri.clone()));
    }

    // 5. Création et insertion dans la base
    mgr.create_collection("exchange-items", Some(db_schema_uri))
        .await
        .expect("Échec création collection");

    match mgr.insert_with_schema("exchange-items", json_doc).await {
        Ok(res) => {
            assert!(res.get("id").is_some());
            println!("✅ Insertion réussie avec l'ID : {}", res["id"]);
        }
        Err(e) => panic!("❌ ÉCHEC INSERTION : {}", e),
    }
}
