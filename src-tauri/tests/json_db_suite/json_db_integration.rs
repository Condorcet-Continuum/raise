// FICHIER : src-tauri/tests/json_db_suite/json_db_integration.rs

use crate::common::{ensure_db_exists, get_dataset_file, init_test_env, TEST_DB, TEST_SPACE};
use genaptitude::json_db::collections::manager::CollectionsManager;
use genaptitude::json_db::storage::JsonDbConfig;
use serde_json::Value;
use std::fs;

fn load_base_article(cfg: &JsonDbConfig) -> Value {
    let article_path = get_dataset_file(cfg, "arcadia/v1/data/articles/article.json");
    if !article_path.exists() {
        panic!("❌ Dataset introuvable : {:?}", article_path);
    }
    let raw = fs::read_to_string(&article_path).unwrap();
    serde_json::from_str(&raw).unwrap()
}

#[test]
fn query_get_article_by_id() {
    let test_env = init_test_env();
    ensure_db_exists(&test_env.cfg, TEST_SPACE, TEST_DB);

    let mgr = CollectionsManager::new(&test_env.storage, TEST_SPACE, TEST_DB);

    // CORRECTION : Construction de l'URI absolue pour le schéma
    let schema_uri = format!(
        "db://{}/{}/schemas/v1/articles/article.schema.json",
        TEST_SPACE, TEST_DB
    );

    mgr.create_collection("articles", Some(schema_uri.clone()))
        .ok();

    let mut json_doc = load_base_article(&test_env.cfg);
    if let Some(obj) = json_doc.as_object_mut() {
        obj.insert(
            "handle".to_string(),
            Value::String("test-handle-get".to_string()),
        );
        obj.insert(
            "slug".to_string(),
            Value::String("test-handle-get".to_string()),
        );
        obj.insert(
            "displayName".to_string(),
            Value::String("Test Handle Get".to_string()),
        );
    }

    // Insertion via schéma
    let stored = mgr
        .insert_with_schema("articles", json_doc)
        .expect("insert article failed");

    let id = stored
        .get("id")
        .and_then(|v| v.as_str())
        .expect("ID manquant après insert");

    let loaded_opt = mgr.get("articles", id).expect("get failed");
    let loaded = loaded_opt.expect("Document non trouvé");

    assert_eq!(loaded.get("id").and_then(|v| v.as_str()), Some(id));
}

#[test]
fn query_find_one_article_by_handle() {
    let test_env = init_test_env();
    ensure_db_exists(&test_env.cfg, TEST_SPACE, TEST_DB);
    let mgr = CollectionsManager::new(&test_env.storage, TEST_SPACE, TEST_DB);

    // CORRECTION : URI absolue
    let schema_uri = format!(
        "db://{}/{}/schemas/v1/articles/article.schema.json",
        TEST_SPACE, TEST_DB
    );
    mgr.create_collection("articles", Some(schema_uri)).ok();

    let base_doc = load_base_article(&test_env.cfg);
    let target_handle = "handle-002";
    let mut target_id = String::new();

    for i in 1..=3 {
        let mut json_doc = base_doc.clone();
        let h = format!("handle-{:03}", i);
        if let Some(obj) = json_doc.as_object_mut() {
            obj.insert("handle".to_string(), Value::String(h.clone()));
            obj.insert("slug".to_string(), Value::String(h.clone()));
            obj.insert(
                "displayName".to_string(),
                Value::String(format!("Display {}", h)),
            );
        }
        let stored = mgr
            .insert_with_schema("articles", json_doc)
            .expect("insert");
        if h == target_handle {
            target_id = stored.get("id").unwrap().as_str().unwrap().to_string();
        }
    }

    let all = mgr.list_all("articles").expect("list_all");
    let found = all
        .iter()
        .find(|d| d.get("handle").and_then(|v| v.as_str()) == Some(target_handle))
        .expect("article non trouvé");

    assert_eq!(found.get("id").unwrap().as_str().unwrap(), target_id);
}

#[test]
fn query_find_many_with_sort_and_limit_simulated() {
    let test_env = init_test_env();
    ensure_db_exists(&test_env.cfg, TEST_SPACE, TEST_DB);
    let mgr = CollectionsManager::new(&test_env.storage, TEST_SPACE, TEST_DB);

    // CORRECTION : URI absolue
    let schema_uri = format!(
        "db://{}/{}/schemas/v1/articles/article.schema.json",
        TEST_SPACE, TEST_DB
    );
    mgr.create_collection("articles", Some(schema_uri)).ok();

    let base_doc = load_base_article(&test_env.cfg);

    for i in 1..=5 {
        let mut json_doc = base_doc.clone();
        if let Some(obj) = json_doc.as_object_mut() {
            let h = format!("sort-{:03}", i);
            obj.insert("handle".to_string(), Value::String(h.clone()));
            obj.insert("slug".to_string(), Value::String(h.clone()));
        }
        mgr.insert_with_schema("articles", json_doc)
            .expect("insert");
    }

    let mut all = mgr.list_all("articles").expect("list_all");
    all.sort_by(|a, b| {
        let ha = a.get("handle").and_then(|v| v.as_str()).unwrap_or("");
        let hb = b.get("handle").and_then(|v| v.as_str()).unwrap_or("");
        hb.cmp(ha)
    });

    let handles: Vec<String> = all
        .iter()
        .take(3)
        .map(|d| d.get("handle").unwrap().as_str().unwrap().to_string())
        .collect();

    assert_eq!(handles, vec!["sort-005", "sort-004", "sort-003"]);
}
