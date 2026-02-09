// FICHIER : src-tauri/tests/json_db_suite/json_db_sql.rs

use crate::{ensure_db_exists, get_dataset_file, init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::query::sql::{parse_sql, SqlRequest};
use raise::json_db::query::QueryEngine;
use raise::json_db::storage::JsonDbConfig;
use raise::utils::{fs, json::json};

async fn seed_actors_from_dataset(mgr: &CollectionsManager<'_>, cfg: &JsonDbConfig) {
    // CORRECTION : URI absolue pour le schéma
    let schema_uri = format!(
        "db://{}/{}/schemas/v1/actors/actor.schema.json",
        TEST_SPACE, TEST_DB
    );

    // CORRECTION E0599 : Ajout de .await sur create_collection
    mgr.create_collection("actors", Some(schema_uri))
        .await
        .expect("create collection actors");

    let actors_data = vec![
        json!({ "handle": "alice", "displayName": "Alice Admin", "kind": "human", "roles": ["admin"], "tags": ["core", "paris"], "x_age": 30, "x_city": "Paris", "x_active": true }),
        json!({ "handle": "bob", "displayName": "Bob User", "kind": "human", "roles": ["editor"], "tags": ["lyon"], "x_age": 25, "x_city": "Lyon", "x_active": true }),
        json!({ "handle": "charlie", "displayName": "Charlie Guest", "kind": "human", "roles": ["guest"], "tags": ["paris"], "x_age": 35, "x_city": "Paris", "x_active": false }),
        json!({ "handle": "bot-build", "displayName": "Build Bot", "kind": "bot", "tags": ["ci"], "x_age": 1, "x_city": "Cloud", "x_active": true }),
        json!({ "handle": "eve", "displayName": "Eve Manager", "kind": "human", "roles": ["admin", "manager"], "x_age": 40, "x_city": "Lyon", "x_active": false }),
        json!({ "handle": "frank", "displayName": "Frank Dev", "kind": "human", "roles": ["dev"], "x_age": 30, "x_city": "Bordeaux", "x_active": true }),
    ];

    for actor in actors_data {
        let handle = actor["handle"].as_str().unwrap();
        let rel_path = format!("actors/{}.json", handle);
        let file_path = get_dataset_file(cfg, &rel_path).await;

        // IMPORTANT : Création du dossier parent (nécessaire car get_dataset_file pointe vers <tmp>/dataset/...)
        if let Some(parent) = file_path.parent() {
            if !fs::exists(parent).await {
                fs::ensure_dir(parent)
                    .await
                    .expect("Failed to create actor dataset dir");
            }
        }
        fs::write_json_atomic(&file_path, &actor)
            .await
            .expect("write dataset file");

        mgr.insert_with_schema("actors", actor)
            .await
            .expect("Failed to insert actor");
    }
}

#[tokio::test]
async fn test_sql_select_by_kind() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, TEST_SPACE, TEST_DB).await;
    let mgr = CollectionsManager::new(&env.storage, TEST_SPACE, TEST_DB);

    seed_actors_from_dataset(&mgr, &env.cfg).await;

    let engine = QueryEngine::new(&mgr);
    let sql = "SELECT * FROM actors WHERE kind = 'bot'";

    // DÉBALLAGE DU SQL REQUEST
    let request = parse_sql(sql).expect("Parsing SQL");
    let query = match request {
        SqlRequest::Read(q) => q,
        _ => panic!("Expected SELECT query"),
    };

    let result = engine.execute_query(query).await.expect("Exec");

    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0]["handle"], "bot-build");
}

#[tokio::test]
async fn test_sql_numeric_comparison_x_props() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, TEST_SPACE, TEST_DB).await;
    let mgr = CollectionsManager::new(&env.storage, TEST_SPACE, TEST_DB);
    seed_actors_from_dataset(&mgr, &env.cfg).await;
    let engine = QueryEngine::new(&mgr);

    let sql = "SELECT * FROM actors WHERE x_age >= 30";

    // DÉBALLAGE
    let request = parse_sql(sql).expect("Parsing SQL");
    let query = match request {
        SqlRequest::Read(q) => q,
        _ => panic!("Expected SELECT query"),
    };

    let result = engine.execute_query(query).await.expect("Exec");

    assert_eq!(result.documents.len(), 4);
}

#[tokio::test]
async fn test_sql_logical_and_mixed() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, TEST_SPACE, TEST_DB).await;
    let mgr = CollectionsManager::new(&env.storage, TEST_SPACE, TEST_DB);
    seed_actors_from_dataset(&mgr, &env.cfg).await;
    let engine = QueryEngine::new(&mgr);

    let sql = "SELECT * FROM actors WHERE kind = 'human' AND x_active = true";

    // DÉBALLAGE
    let request = parse_sql(sql).expect("Parsing SQL");
    let query = match request {
        SqlRequest::Read(q) => q,
        _ => panic!("Expected SELECT query"),
    };

    let result = engine.execute_query(query).await.expect("Exec");

    assert_eq!(result.documents.len(), 3);
}

#[tokio::test]
async fn test_sql_like_display_name() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, TEST_SPACE, TEST_DB).await;
    let mgr = CollectionsManager::new(&env.storage, TEST_SPACE, TEST_DB);
    seed_actors_from_dataset(&mgr, &env.cfg).await;
    let engine = QueryEngine::new(&mgr);

    let sql = "SELECT * FROM actors WHERE displayName LIKE 'User'";

    // DÉBALLAGE
    let request = parse_sql(sql).expect("Parsing SQL");
    let query = match request {
        SqlRequest::Read(q) => q,
        _ => panic!("Expected SELECT query"),
    };

    let result = engine.execute_query(query).await.expect("Exec");

    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0]["handle"], "bob");
}

#[tokio::test]
async fn test_sql_order_by_x_prop() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, TEST_SPACE, TEST_DB).await;
    let mgr = CollectionsManager::new(&env.storage, TEST_SPACE, TEST_DB);
    seed_actors_from_dataset(&mgr, &env.cfg).await;
    let engine = QueryEngine::new(&mgr);

    // SQL : On veut les 2 plus âgés.
    let sql = "SELECT * FROM actors ORDER BY x_age DESC LIMIT 2";

    // DÉBALLAGE
    let request = parse_sql(sql).expect("Parsing SQL");
    let query = match request {
        SqlRequest::Read(q) => q,
        _ => panic!("Expected SELECT query"),
    };

    let result = engine.execute_query(query).await.expect("Exec");

    // On vérifie que l'on a AU MOINS 2 résultats et que l'ordre est correct.
    assert!(
        result.documents.len() >= 2,
        "Doit retourner au moins 2 résultats"
    );

    // Eve (40 ans) doit être première
    assert_eq!(result.documents[0]["handle"], "eve");
    // Charlie (35 ans) doit être second
    assert_eq!(result.documents[1]["handle"], "charlie");
}

#[tokio::test]
async fn test_sql_json_array_contains() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, TEST_SPACE, TEST_DB).await;
    let mgr = CollectionsManager::new(&env.storage, TEST_SPACE, TEST_DB);
    seed_actors_from_dataset(&mgr, &env.cfg).await;
    let engine = QueryEngine::new(&mgr);

    let sql = "SELECT * FROM actors WHERE tags LIKE 'paris'";

    // DÉBALLAGE
    let request = parse_sql(sql).expect("Parsing SQL");
    let query = match request {
        SqlRequest::Read(q) => q,
        _ => panic!("Expected SELECT query"),
    };

    let result = engine.execute_query(query).await.expect("Exec");

    assert_eq!(result.documents.len(), 2);
}
