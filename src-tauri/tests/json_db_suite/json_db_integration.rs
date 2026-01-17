// FICHIER : src-tauri/tests/json_db_suite/json_db_integration.rs

use crate::{ensure_db_exists, init_test_env};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::StorageEngine;
use serde_json::json;

#[tokio::test] // CORRECTION : Passage en test asynchrone pour supporter les appels .await
async fn query_get_article_by_id() {
    // CORRECTION E0277 : Ces helpers sont synchrones dans cette suite de tests
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    // CORRECTION E0599 : Méthode asynchrone, ajout de .await avant .expect()
    mgr.create_collection("articles", None)
        .await
        .expect("create collection");

    let doc = json!({
        "handle": "my-article",
        "slug": "my-article",
        "displayName": "Mon Article",
        "title": "Titre Obligatoire",
        "status": "published"
    });

    let inserted = mgr
        .insert_with_schema("articles", doc)
        .await
        .expect("insert article failed");

    let id = inserted
        .get("id")
        .and_then(|v| v.as_str())
        .expect("id manquant");

    // CORRECTION E0599 : get() est désormais asynchrone
    let fetched = mgr
        .get("articles", id)
        .await
        .expect("get failed")
        .expect("document non trouvé");

    assert_eq!(fetched.get("handle").unwrap(), "my-article");
    assert_eq!(fetched.get("slug").unwrap(), "my-article");
    assert_eq!(fetched.get("title").unwrap(), "Titre Obligatoire");
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn query_find_one_article_by_handle() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    mgr.create_collection("articles", None).await.unwrap();

    let doc1 = json!({
        "handle": "a1",
        "slug": "a1",
        "displayName": "A1",
        "title": "Titre A1",
        "status": "draft"
    });
    let doc2 = json!({
        "handle": "a2",
        "slug": "a2",
        "displayName": "A2",
        "title": "Titre A2",
        "status": "published"
    });

    mgr.insert_with_schema("articles", doc1)
        .await
        .expect("insert");
    mgr.insert_with_schema("articles", doc2)
        .await
        .expect("insert");

    // CORRECTION E0599 : list_all() est désormais asynchrone
    let all = mgr.list_all("articles").await.unwrap();
    let found = all
        .into_iter()
        .find(|d| d.get("handle").and_then(|s| s.as_str()) == Some("a2"));

    assert!(found.is_some());
    assert_eq!(found.unwrap().get("status").unwrap(), "published");
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn query_find_many_with_sort_and_limit_simulated() {
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    mgr.create_collection("articles", None).await.unwrap();

    for i in 0..5 {
        let doc = json!({
            "handle": format!("handle-{}", i),
            "slug": format!("handle-{}", i),
            "displayName": format!("Article {}", i),
            "title": format!("Titre {}", i),
            "status": "published"
        });
        // CORRECTION E0599 : .await nécessaire dans la boucle d'insertion
        mgr.insert_with_schema("articles", doc)
            .await
            .expect("insert");
    }

    let all = mgr.list_all("articles").await.unwrap();
    assert_eq!(all.len(), 5);
}
