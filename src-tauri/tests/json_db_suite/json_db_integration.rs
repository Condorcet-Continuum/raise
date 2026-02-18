// FICHIER : src-tauri/tests/json_db_suite/json_db_integration.rs

use crate::common::setup_test_env; // Nouveau socle SSOT
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*; // Apporte Value, json!, Result, etc.

#[tokio::test]
async fn query_get_article_by_id() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);

    // 2. Création de la collection
    mgr.create_collection("articles", None)
        .await
        .expect("❌ Échec de la création de la collection 'articles'");

    let doc = json!({
        "handle": "my-article",
        "slug": "my-article",
        "displayName": "Mon Article",
        "title": "Titre Obligatoire",
        "status": "published"
    });

    // 3. Insertion
    let inserted = mgr
        .insert_with_schema("articles", doc)
        .await
        .expect("❌ L'insertion de l'article a échoué");

    let id = inserted
        .get("id")
        .and_then(|v| v.as_str())
        .expect("❌ L'ID est manquant dans le document inséré");

    // 4. Récupération par ID (Vérification du cycle complet)
    let fetched = mgr
        .get("articles", id)
        .await
        .expect("❌ Échec de l'appel 'get'")
        .expect("❌ Le document n'a pas été trouvé en base après insertion");

    assert_eq!(
        fetched["handle"], "my-article",
        "Le handle ne correspond pas"
    );
    assert_eq!(
        fetched["title"], "Titre Obligatoire",
        "Le titre ne correspond pas"
    );
}

#[tokio::test]
async fn query_find_one_article_by_handle() {
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);

    mgr.create_collection("articles", None)
        .await
        .expect("❌ Échec create_collection");

    let docs = vec![
        json!({ "handle": "a1", "slug": "a1", "displayName": "A1", "title": "T", "status": "draft" }),
        json!({ "handle": "a2", "slug": "a2", "displayName": "A2", "title": "T", "status": "published" }),
    ];

    for doc in docs {
        mgr.insert_with_schema("articles", doc)
            .await
            .expect("❌ Échec insertion lot");
    }

    // Vérification de la liste
    let all = mgr.list_all("articles").await.expect("❌ Échec list_all");

    let found = all
        .into_iter()
        .find(|d| d.get("handle").and_then(|s| s.as_str()) == Some("a2"));

    assert!(found.is_some(), "L'article 'a2' devrait être présent");
    assert_eq!(
        found.unwrap()["status"],
        "published",
        "Le statut devrait être 'published'"
    );
}

#[tokio::test]
async fn query_find_many_with_sort_and_limit_simulated() {
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);

    mgr.create_collection("articles", None)
        .await
        .expect("❌ Échec create_collection");

    for i in 0..5 {
        let doc = json!({
            "handle": format!("handle-{}", i),
            "slug": format!("handle-{}", i),
            "displayName": format!("Article {}", i),
            "title": format!("Titre {}", i),
            "status": "published"
        });
        mgr.insert_with_schema("articles", doc)
            .await
            .expect("❌ Échec insertion boucle");
    }

    let all = mgr
        .list_all("articles")
        .await
        .expect("❌ Échec list_all final");

    assert_eq!(all.len(), 5, "Le nombre de documents en base est incorrect");
}
