// FICHIER : src-tauri/tests/json_db_suite/dataset_integration.rs

use crate::{ensure_db_exists, get_dataset_file, init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::StorageEngine;
use raise::utils::{fs, json};

#[tokio::test] // On garde tokio pour les appels asynchrones au manager
async fn debug_import_exchange_item() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, &env.space, &env.db).await;

    let refreshed_storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&refreshed_storage, &env.space, &env.db);

    // Le fichier est créé par init_test_env dans le mod.rs
    let data_path =
        get_dataset_file(&env.cfg, "arcadia/v1/data/exchange-items/position_gps.json").await;

    // Vérification de sécurité
    if !fs::exists(&data_path).await {
        panic!("❌ Fichier de test introuvable : {:?}", data_path);
    }

    let json_content = fs::read_to_string(&data_path)
        .await
        .expect("Lecture donnée impossible");

    // Utilisation de la variable définie juste au-dessus
    let mut json_doc: json::Value = json::parse(&json_content).expect("JSON malformé");

    // On utilise un schéma qui existe réellement (copié par init_test_env)
    let schema_rel_path = "arcadia/data/exchange-item.schema.json";
    let db_schema_uri = format!(
        "db://{}/{}/schemas/v1/{}",
        TEST_SPACE, TEST_DB, schema_rel_path
    );

    if let Some(obj) = json_doc.as_object_mut() {
        obj.insert(
            "$schema".to_string(),
            serde_json::Value::String(db_schema_uri.clone()),
        );
    }

    // Le CollectionsManager est asynchrone, on conserve donc les .await ici.
    mgr.create_collection("exchange-items", Some(db_schema_uri))
        .await
        .expect("create collection");

    match mgr.insert_with_schema("exchange-items", json_doc).await {
        Ok(res) => {
            assert!(res.get("id").is_some());
        }
        Err(e) => panic!("❌ ÉCHEC INSERTION : {}", e),
    }
}
