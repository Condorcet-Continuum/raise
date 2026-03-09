// FICHIER : src-tauri/tests/json_db_suite/dataset_integration.rs

use crate::common::{seed_mock_datasets, setup_test_env, LlmMode};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*; // SSOT : Apporte JsonValue, json, Result, etc.

#[async_test]
async fn debug_import_exchange_item() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env(LlmMode::Disabled).await;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);

    // 2. Création du fichier factice (remplace l'ancienne méthode)
    let data_path = seed_mock_datasets(&env.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap())
        .await
        .unwrap();

    // 3. Lecture du fichier via notre SSOT
    let mut json_doc: JsonValue = fs::read_json_async(&data_path)
        .await
        .expect("Lecture ou parsing JSON impossible");

    // 4. Injection du schéma
    let schema_rel_path = "db/generic.schema.json";
    let db_schema_uri = format!(
        "db://{}/{}/schemas/v1/{}",
        env.space, env.db, schema_rel_path
    );

    if let Some(obj) = json_doc.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            JsonValue::String(db_schema_uri.clone()),
        );
    }

    // 5. Création et insertion dans la base
    mgr.create_collection("exchange-items", &db_schema_uri)
        .await
        .expect("Échec création collection");

    match mgr.insert_with_schema("exchange-items", json_doc).await {
        Ok(res) => {
            assert!(res.get("_id").is_some());
            println!("✅ Insertion réussie avec l'ID : {}", res["_id"]);
        }
        Err(e) => panic!("❌ ÉCHEC INSERTION : {}", e),
    }
}
