// FICHIER : src-tauri/tests/json_db_suite/json_db_lifecycle.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::file_storage::{create_db, drop_db, open_db, DropMode};
use raise::json_db::storage::JsonDbConfig;
// ✅ Imports nettoyés et précisés
use raise::utils::json::json;
use serde_json::Value;

#[tokio::test]
async fn db_lifecycle_minimal() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = JsonDbConfig {
        data_root: env.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap(),
    };

    let space = "lifecycle_minimal";
    let db = "test_db";

    let system_doc = json!({
        "collections": { "actors": { "items": [] } },
        "rules": { "_system_rules": { "items": [] } },
        "schemas": { "v1": {} }
    });

    create_db(&cfg, space, db, &system_doc)
        .await
        .expect("❌ create_db doit réussir");

    let db_root = cfg.db_root(space, db);
    assert!(db_root.is_dir());

    open_db(&cfg, space, db)
        .await
        .expect("❌ open_db doit réussir");

    drop_db(&cfg, space, db, DropMode::Soft)
        .await
        .expect("❌ drop_db soft doit réussir");
    assert!(!db_root.exists());

    create_db(&cfg, space, db, &system_doc)
        .await
        .expect("❌ recreate_db doit réussir");
    drop_db(&cfg, space, db, DropMode::Hard)
        .await
        .expect("❌ drop_db hard doit réussir");
}

#[tokio::test]
async fn test_collection_drop_cleans_system_index() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let collection = "temp_collection_to_drop";

    mgr.create_collection(
        collection,
        "db://_system/_system/schemas/v1/db/generic.schema.json",
    )
    .await
    .expect("❌ Échec création");

    // 🎯 FIX : Annotation de type explicite pour lever l'ambiguïté
    let sys_json: Value = mgr.load_index().await.expect("❌ Lecture via manager");
    assert!(sys_json
        .pointer(&format!("/collections/{}", collection))
        .is_some());

    mgr.drop_collection(collection)
        .await
        .expect("❌ drop_collection a échoué");

    let sys_json_after: Value = mgr.load_index().await.expect("❌ Lecture finale");
    assert!(sys_json_after
        .pointer(&format!("/collections/{}", collection))
        .is_none());
}

#[tokio::test]
async fn test_system_index_strict_conformance() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);

    mgr.create_collection(
        "init_trigger",
        "db://_system/_system/schemas/v1/db/generic.schema.json",
    )
    .await
    .unwrap();

    // 🎯 FIX : Annotation de type explicite
    let doc: Value = mgr
        .load_index()
        .await
        .expect("❌ L'index doit être lisible");

    assert!(doc.get("_id").is_some());

    let expected_schema = "db://_system/_system/schemas/v1/db/index.schema.json";

    assert_eq!(
        doc.get("$schema").and_then(|v| v.as_str()),
        Some(expected_schema)
    );

    let registry = SchemaRegistry::from_db(&env.sandbox.storage.config, &env.space, &env.db)
        .await
        .unwrap();
    let validator = SchemaValidator::compile_with_registry(expected_schema, &registry).unwrap();

    validator
        .validate(&doc)
        .expect("❌ Non-conformité détectée");
}
