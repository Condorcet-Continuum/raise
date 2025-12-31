// FICHIER : src-tauri/tests/json_db_suite/json_db_integration.rs

use crate::{ensure_db_exists, init_test_env};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::StorageEngine;
use serde_json::json;

#[test]
fn query_get_article_by_id() {
    let env = init_test_env();
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    mgr.create_collection("articles", None)
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
        .expect("insert article failed");

    let id = inserted
        .get("id")
        .and_then(|v| v.as_str())
        .expect("id manquant");

    let fetched = mgr
        .get("articles", id)
        .expect("get failed")
        .expect("document non trouvé");

    assert_eq!(fetched.get("handle").unwrap(), "my-article");
    assert_eq!(fetched.get("slug").unwrap(), "my-article");
    assert_eq!(fetched.get("title").unwrap(), "Titre Obligatoire");
}

#[test]
fn query_find_one_article_by_handle() {
    let env = init_test_env();
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    mgr.create_collection("articles", None).unwrap();

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

    mgr.insert_with_schema("articles", doc1).expect("insert");
    mgr.insert_with_schema("articles", doc2).expect("insert");

    let all = mgr.list_all("articles").unwrap();
    let found = all
        .into_iter()
        .find(|d| d.get("handle").and_then(|s| s.as_str()) == Some("a2"));

    assert!(found.is_some());
    assert_eq!(found.unwrap().get("status").unwrap(), "published");
}

#[test]
fn query_find_many_with_sort_and_limit_simulated() {
    let env = init_test_env();
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    mgr.create_collection("articles", None).unwrap();

    for i in 0..5 {
        let doc = json!({
            "handle": format!("handle-{}", i),
            "slug": format!("handle-{}", i),
            "displayName": format!("Article {}", i),
            "title": format!("Titre {}", i),
            "status": "published"
            // SUPPRESSION : Pas de champ x_... inutile ici car le schéma est strict
        });
        mgr.insert_with_schema("articles", doc).expect("insert");
    }

    let all = mgr.list_all("articles").unwrap();
    assert_eq!(all.len(), 5);
}
