// FICHIER : src-tauri/tests/json_db_suite/json_db_query_integration.rs

use crate::{ensure_db_exists, get_dataset_file, init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::{
    collections::manager::CollectionsManager,
    query::{
        ComparisonOperator, Condition, FilterOperator, Query, QueryEngine, QueryFilter, SortField,
        SortOrder,
    },
    storage::JsonDbConfig,
};
use raise::utils::{
    fs,
    json::{self, json, Value},
};

async fn load_test_doc(cfg: &JsonDbConfig) -> Value {
    let path = get_dataset_file(cfg, "arcadia/v1/data/articles/article.json").await;
    if !fs::exists(&path).await {
        panic!("❌ Dataset article.json introuvable : {:?}", path);
    }
    let raw = fs::read_to_string(&path).await.expect("Lecture impossible");
    json::parse(&raw).expect("JSON invalide")
}

// CORRECTION : Passage en async pour supporter les appels asynchrones au manager
async fn seed_article<'a>(
    mgr: &'a CollectionsManager<'a>,
    handle: &str,
    doc_template: &Value,
) -> String {
    let mut doc = doc_template.clone();
    if let Some(obj) = doc.as_object_mut() {
        obj.remove("id");
        obj.insert("handle".to_string(), Value::String(handle.to_string()));
        obj.insert("slug".to_string(), Value::String(handle.to_string()));
        obj.insert(
            "displayName".to_string(),
            Value::String(format!("Display {}", handle)),
        );
        obj.insert(
            "title".to_string(),
            Value::String(format!("Titre de l'article {}", handle)),
        );
        obj.insert(
            "authorId".to_string(),
            Value::String("00000000-0000-0000-0000-000000000000".to_string()),
        );
    }

    let schema_uri = format!(
        "db://{}/{}/schemas/v1/articles/article.schema.json",
        TEST_SPACE, TEST_DB
    );

    // CORRECTION E0599 : Ajout de .await sur create_collection
    mgr.create_collection("articles", Some(schema_uri))
        .await
        .ok();

    // CORRECTION E0599 : Ajout de .await sur insert_with_schema
    let stored = mgr
        .insert_with_schema("articles", doc)
        .await
        .expect("insert failed");

    stored.get("id").unwrap().as_str().unwrap().to_string()
}

#[tokio::test]
async fn query_get_article_by_id() {
    // CORRECTION E0277 : Ces helpers sont synchrones dans cette suite
    let test_env = init_test_env().await;
    ensure_db_exists(&test_env.cfg, TEST_SPACE, TEST_DB).await;

    let mgr = CollectionsManager::new(&test_env.storage, TEST_SPACE, TEST_DB);
    let base_doc = load_test_doc(&test_env.cfg).await;

    let handle = "query-get-id";
    // CORRECTION : seed_article est désormais asynchrone
    let id = seed_article(&mgr, handle, &base_doc).await;

    // CORRECTION E0599 : get() est désormais asynchrone
    let loaded_opt = mgr.get("articles", &id).await.expect("get failed");
    let loaded = loaded_opt.expect("Document non trouvé");
    assert_eq!(loaded.get("handle").unwrap().as_str(), Some(handle));
}

#[tokio::test]
async fn query_find_one_article_by_handle() {
    let test_env = init_test_env().await;
    ensure_db_exists(&test_env.cfg, TEST_SPACE, TEST_DB).await;

    let mgr = CollectionsManager::new(&test_env.storage, TEST_SPACE, TEST_DB);
    let base_doc = load_test_doc(&test_env.cfg).await;

    let handle = "query-find-one";
    // CORRECTION : .await sur seed_article
    seed_article(&mgr, handle, &base_doc).await;

    let engine = QueryEngine::new(&mgr);
    let filter = QueryFilter {
        operator: FilterOperator::And,
        conditions: vec![Condition {
            field: "handle".to_string(),
            operator: ComparisonOperator::Eq,
            value: json!(handle),
        }],
    };
    let query = Query {
        collection: "articles".to_string(),
        filter: Some(filter),
        sort: None,
        limit: Some(1),
        offset: None,
        projection: None,
    };

    let result = engine.execute_query(query).await.expect("query failed");
    assert!(!result.documents.is_empty());
    assert_eq!(
        result.documents[0].get("handle").unwrap().as_str(),
        Some(handle)
    );
}

#[tokio::test]
async fn query_find_many_with_sort_and_limit() {
    let test_env = init_test_env().await;
    ensure_db_exists(&test_env.cfg, TEST_SPACE, TEST_DB).await;

    let mgr = CollectionsManager::new(&test_env.storage, TEST_SPACE, TEST_DB);
    let base_doc = load_test_doc(&test_env.cfg).await;

    // On insère 10 articles : sort-0 ... sort-9
    for i in 0..10 {
        // CORRECTION : .await sur seed_article dans la boucle
        seed_article(&mgr, &format!("sort-{}", i), &base_doc).await;
    }

    let engine = QueryEngine::new(&mgr);
    let q = Query {
        collection: "articles".to_string(),
        filter: None,
        // CORRECTION : Tri sur "handle" (Descendant) au lieu de "x_price"
        sort: Some(vec![SortField {
            field: "handle".to_string(),
            order: SortOrder::Desc,
        }]),
        offset: Some(0),
        limit: Some(3),
        projection: None,
    };

    let result = engine.execute_query(q).await.expect("query failed");

    assert_eq!(result.documents.len(), 3);

    // "sort-9" est le plus grand alphabétiquement
    assert_eq!(
        result.documents[0].get("handle").unwrap().as_str(),
        Some("sort-9")
    );
}
