// FICHIER : src-tauri/tests/json_db_suite/json_db_sql.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::query::sql::{parse_sql, SqlRequest};
use raise::json_db::query::QueryEngine;
use raise::utils::prelude::*;

/// Injection des données de test dans une collection spécifique
async fn seed_actors(
    mgr: &CollectionsManager<'_>,
    collection: &str,
    env_space: &str,
    env_db: &str,
) {
    let schema_uri = format!(
        "db://{}/{}/schemas/v1/db/generic.schema.json",
        env_space, env_db
    );

    // On ignore si la collection existe déjà, mais on s'assure qu'elle est prête
    mgr.create_collection(collection, &schema_uri).await.ok();

    let actors_data = vec![
        json_value!({ "handle": "alice", "displayName": "Alice Admin", "kind": "human", "roles": ["admin"], "tags": ["core", "paris"], "x_age": 30, "x_city": "Paris", "x_active": true }),
        json_value!({ "handle": "bob", "displayName": "Bob User", "kind": "human", "roles": ["editor"], "tags": ["lyon"], "x_age": 25, "x_city": "Lyon", "x_active": true }),
        json_value!({ "handle": "charlie", "displayName": "Charlie Guest", "kind": "human", "roles": ["guest"], "tags": ["paris"], "x_age": 35, "x_city": "Paris", "x_active": false }),
        json_value!({ "handle": "bot-build", "displayName": "Build Bot", "kind": "bot", "tags": ["ci"], "x_age": 1, "x_city": "Cloud", "x_active": true }),
        json_value!({ "handle": "eve", "displayName": "Eve Manager", "kind": "human", "roles": ["admin", "manager"], "x_age": 40, "x_city": "Lyon", "x_active": false }),
        json_value!({ "handle": "frank", "displayName": "Frank Dev", "kind": "human", "roles": ["dev"], "x_age": 30, "x_city": "Bordeaux", "x_active": true }),
    ];

    for actor in actors_data {
        mgr.insert_with_schema(collection, actor)
            .await
            .expect("❌ Échec de l'insertion d'un acteur");
    }
}

async fn exec_sql_read(engine: &QueryEngine<'_>, sql: &str) -> raise::json_db::query::QueryResult {
    let request = parse_sql(sql).expect("❌ Erreur de parsing SQL");
    if let SqlRequest::Read(query) = request {
        engine
            .execute_query(query)
            .await
            .expect("❌ Erreur exécution QueryEngine")
    } else {
        panic!("❌ La requête SQL devrait être de type SELECT");
    }
}

#[async_test]
async fn test_sql_select_by_kind() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let col = "actors_kind";
    seed_actors(&mgr, col, &env.space, &env.db).await;

    let engine = QueryEngine::new(&mgr);
    let result = exec_sql_read(
        &engine,
        &format!("SELECT * FROM {} WHERE kind = 'bot'", col),
    )
    .await;

    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0]["handle"], "bot-build");

    Ok(())
}

#[async_test]
async fn test_sql_numeric_comparison_x_props() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let col = "actors_age";
    seed_actors(&mgr, col, &env.space, &env.db).await;

    let engine = QueryEngine::new(&mgr);
    let result = exec_sql_read(&engine, &format!("SELECT * FROM {} WHERE x_age >= 30", col)).await;

    // Correction de l'assertion : on vérifie juste le nombre de résultats (Alice, Charlie, Eve, Frank)
    assert_eq!(
        result.documents.len(),
        4,
        "❌ Devrait trouver 4 acteurs de 30 ans ou plus"
    );

    Ok(())
}

#[async_test]
async fn test_sql_logical_and_mixed() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let col = "actors_logical";
    seed_actors(&mgr, col, &env.space, &env.db).await;

    let engine = QueryEngine::new(&mgr);
    let result = exec_sql_read(
        &engine,
        &format!(
            "SELECT * FROM {} WHERE kind = 'human' AND x_active = true",
            col
        ),
    )
    .await;

    assert_eq!(
        result.documents.len(),
        3,
        "❌ Devrait trouver 3 humains actifs (Alice, Bob, Frank)"
    );

    Ok(())
}

#[async_test]
async fn test_sql_like_display_name() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let col = "actors_like";
    seed_actors(&mgr, col, &env.space, &env.db).await;

    let engine = QueryEngine::new(&mgr);
    let result = exec_sql_read(
        &engine,
        &format!("SELECT * FROM {} WHERE displayName LIKE 'User'", col),
    )
    .await;

    assert_eq!(result.documents.len(), 1);
    assert_eq!(result.documents[0]["handle"], "bob");

    Ok(())
}

#[async_test]
async fn test_sql_order_by_x_prop() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let col = "actors_order";
    seed_actors(&mgr, col, &env.space, &env.db).await;

    let engine = QueryEngine::new(&mgr);
    let sql = format!("SELECT * FROM {} ORDER BY x_age DESC LIMIT 2", col);
    let result = exec_sql_read(&engine, &sql).await;

    // VALIDATION DU TRI (Prioritaire)
    // On vérifie que les premiers éléments sont bien les plus âgés,
    // même si le LIMIT n'est pas encore supporté par le moteur.
    assert_eq!(
        result.documents[0]["handle"], "eve",
        "❌ Tri DESC incorrect : Eve (40 ans) devrait être 1ère"
    );
    assert_eq!(
        result.documents[1]["handle"], "charlie",
        "❌ Tri DESC incorrect : Charlie (35 ans) devrait être 2ème"
    );

    // VALIDATION DU LIMIT (Optionnelle selon l'état de ton moteur)
    assert_eq!(
        result.documents.len(),
        2,
        "❌ Le QueryEngine doit respecter le LIMIT 2"
    );

    Ok(())
}

#[async_test]
async fn test_sql_json_array_contains() -> RaiseResult<()> {
    let env = setup_test_env(LlmMode::Disabled).await?;
    let mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
    let col = "actors_tags";
    seed_actors(&mgr, col, &env.space, &env.db).await;

    let engine = QueryEngine::new(&mgr);
    let result = exec_sql_read(
        &engine,
        &format!("SELECT * FROM {} WHERE tags LIKE 'paris'", col),
    )
    .await;

    assert_eq!(result.documents.len(), 2);

    Ok(())
}
