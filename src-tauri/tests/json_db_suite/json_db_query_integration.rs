// FICHIER : src-tauri/tests/json_db_suite/json_db_query_integration.rs

use crate::common::{seed_mock_datasets, setup_test_env, LlmMode};
use raise::json_db::{
    collections::manager::CollectionsManager,
    query::{
        ComparisonOperator, Condition, FilterOperator, Query, QueryEngine, QueryFilter, SortField,
        SortOrder,
    },
};
use raise::utils::io::{self};
use raise::utils::prelude::*;

/// Helper local pour charger un document de test depuis le mock dataset isolé
async fn load_test_doc(domain_path: &Path) -> Value {
    // On s'assure que les données de test sont présentes dans CE domaine isolé
    let dataset_file = seed_mock_datasets(domain_path)
        .await
        .expect("❌ Échec de la génération des mock datasets");

    io::read_json(&dataset_file)
        .await
        .expect("❌ Lecture du mock JSON impossible")
}

/// Helper pour insérer un article avec un handle spécifique
async fn seed_article(
    mgr: &CollectionsManager<'_>,
    handle: &str,
    doc_template: &Value,
    env_space: &str,
    env_db: &str,
) -> String {
    let mut doc = doc_template.clone();
    if let Some(obj) = doc.as_object_mut() {
        obj.remove("id");
        obj.remove("name");
        obj.remove("exchangeMechanism");

        obj.insert("handle".to_string(), json!(handle));
        obj.insert("slug".to_string(), json!(handle));
        obj.insert(
            "displayName".to_string(),
            json!(format!("Display {}", handle)),
        );
        obj.insert("title".to_string(), json!(format!("Titre {}", handle)));
        obj.insert(
            "authorId".to_string(),
            json!("00000000-0000-0000-0000-000000000000"),
        );
    }

    let schema_uri = format!(
        "db://{}/{}/schemas/v1/articles/article.schema.json",
        env_space, env_db
    );

    // On s'assure que la collection existe
    mgr.create_collection("articles", Some(schema_uri))
        .await
        .ok();

    let stored = mgr
        .insert_with_schema("articles", doc)
        .await
        .expect("❌ Échec insertion article de test");

    stored["id"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn query_get_article_by_id() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let base_doc = load_test_doc(&env.domain_path).await;

    let handle = "query-get-id";
    let id = seed_article(&mgr, handle, &base_doc, &env.space, &env.db).await;

    let loaded = mgr
        .get("articles", &id)
        .await
        .expect("❌ Échec get")
        .expect("❌ Document non trouvé après insertion");

    assert_eq!(loaded["handle"], handle);
}

#[tokio::test]
async fn query_find_one_article_by_handle() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let base_doc = load_test_doc(&env.domain_path).await;

    let handle = "query-find-one";
    seed_article(&mgr, handle, &base_doc, &env.space, &env.db).await;

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

    let result = engine
        .execute_query(query)
        .await
        .expect("❌ Requête find_one échouée");
    assert!(!result.documents.is_empty(), "❌ Aucun document retourné");
    assert_eq!(result.documents[0]["handle"], handle);
}

#[tokio::test]
async fn query_find_many_with_sort_and_limit() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let base_doc = load_test_doc(&env.domain_path).await;

    // Insertion de 10 articles : sort-0 à sort-9
    for i in 0..10 {
        seed_article(&mgr, &format!("sort-{}", i), &base_doc, &env.space, &env.db).await;
    }

    let engine = QueryEngine::new(&mgr);
    let q = Query {
        collection: "articles".to_string(),
        filter: None,
        sort: Some(vec![SortField {
            field: "handle".to_string(),
            order: SortOrder::Desc,
        }]),
        offset: Some(0),
        limit: Some(3),
        projection: None,
    };

    let result = engine
        .execute_query(q)
        .await
        .expect("❌ Requête sort/limit échouée");

    assert_eq!(
        result.documents.len(),
        3,
        "❌ La limite de 3 n'est pas respectée"
    );
    // "sort-9" est le premier dans un tri descendant alphabétique
    assert_eq!(
        result.documents[0]["handle"], "sort-9",
        "❌ Le tri descendant a échoué"
    );
}
