// FICHIER : src-tauri/tests/json_db_suite/schema_minimal.rs

use crate::{ensure_db_exists, init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
// use raise::json_db::storage::file_storage; // Plus nécessaire
use serde_json::json;

#[tokio::test]
async fn schema_instantiate_validate_minimal() {
    // init_test_env et ensure_db_exists sont synchrones dans cette suite
    let test_env = init_test_env().await;
    let cfg = &test_env.cfg;

    let space = TEST_SPACE;
    let db = TEST_DB;

    // 1) S'assurer que la DB existe et contient les schémas
    ensure_db_exists(cfg, space, db);

    // 2) Charger le registre
    let reg = SchemaRegistry::from_db(cfg, space, db).expect("registry from DB");

    // URI du schéma à tester
    let root_uri = reg.uri("actors/actor.schema.json");

    if reg.get_by_uri(&root_uri).is_none() {
        panic!("Schéma introuvable dans le registre de test: {}", root_uri);
    }

    let validator =
        SchemaValidator::compile_with_registry(&root_uri, &reg).expect("compile failed");

    // 3) Document minimal avec TOUS les champs requis (Id, Dates, $schema)
    let mut doc = json!({
      "$schema": root_uri, // Requis
      "id": uuid::Uuid::new_v4().to_string(), // Requis
      "createdAt": chrono::Utc::now().to_rfc3339(), // Requis
      "updatedAt": chrono::Utc::now().to_rfc3339(), // Requis

      "handle": "devops-engineer",
      "displayName": "Ingénieur DevOps",
      "label": { "fr": "Ingénieur DevOps", "en": "DevOps Engineer" },
      "emoji": "🛠️",
      "kind": "human",
      "tags": ["core"]
    });

    // 4) Compute + Validate
    // CORRECTION E0277 : compute_then_validate est synchrone. Suppression de .await
    validator
        .compute_then_validate(&mut doc)
        .expect("compute + validate failed");

    // 5) Vérifications
    assert!(doc.get("id").is_some(), "id manquant");
    assert!(doc.get("createdAt").is_some(), "createdAt manquant");
    assert!(doc.get("updatedAt").is_some(), "updatedAt manquant");
}
