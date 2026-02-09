// FICHIER : src-tauri/tests/json_db_suite/integration_suite.rs

use raise::json_db::{
    collections::manager::CollectionsManager,
    indexes::manager::IndexManager,
    query::{sql, QueryEngine},
    storage::{JsonDbConfig, StorageEngine},
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::utils::{fs::tempdir, json::json, Arc};

#[tokio::test]
async fn test_json_db_global_scenario() {
    // Renommé pour matcher le filtre 'json_db'
    // 1. SETUP ENVIRONNEMENT
    let dir = tempdir().unwrap();
    let config = Arc::new(JsonDbConfig {
        data_root: dir.path().to_path_buf(),
    });
    let space = "integration";
    let db = "test_db";

    let storage = StorageEngine::new((*config).clone());
    let col_mgr = CollectionsManager::new(&storage, space, db);
    let mut idx_mgr = IndexManager::new(&storage, space, db);
    let tx_mgr = TransactionManager::new(&config, space, db);

    // Initialisation DB
    col_mgr.init_db().await.unwrap();

    // 2. CRÉATION SCHÉMA & INDEX
    println!("--- Step 1: Create Collection & Index ---");
    col_mgr.create_collection("users", None).await.unwrap();
    idx_mgr
        .create_index("users", "email", "hash")
        .await
        .unwrap();
    idx_mgr.create_index("users", "age", "btree").await.unwrap();

    // 3. INSERTION VIA TRANSACTION (API Smart)
    println!("--- Step 2: Insert Data (Transaction) ---");
    let tx_reqs = vec![
        TransactionRequest::Insert {
            collection: "users".into(),
            id: Some("u1".into()),
            document: json!({ "name": "Alice", "email": "alice@corp.com", "age": 30 }),
        },
        TransactionRequest::Insert {
            collection: "users".into(),
            id: Some("u2".into()),
            document: json!({ "name": "Bob", "email": "bob@corp.com", "age": 40 }),
        },
    ];
    tx_mgr.execute_smart(tx_reqs).await.unwrap();

    // 4. VERIFICATION VIA SQL (SELECT)
    println!("--- Step 3: Query Data (SQL SELECT) ---");
    // On requête sur l'email (Index Hash)
    let sql_read = "SELECT name, age FROM users WHERE email = 'alice@corp.com'";

    // Déballage du SqlRequest (Select)
    let query = match sql::parse_sql(sql_read).unwrap() {
        sql::SqlRequest::Read(q) => q,
        _ => panic!("Should be Read"),
    };

    let engine = QueryEngine::new(&col_mgr);
    let res = engine.execute_query(query).await.unwrap();
    assert_eq!(res.documents.len(), 1);
    assert_eq!(res.documents[0]["name"], "Alice");
    // Vérif projection
    assert!(res.documents[0].get("email").is_none());

    // 5. MODIFICATION VIA SQL (INSERT)
    println!("--- Step 4: Write Data (SQL INSERT) ---");
    let sql_write =
        "INSERT INTO users (name, email, age) VALUES ('Charlie', 'charlie@corp.com', 25)";

    // Déballage du SqlRequest (Write)
    match sql::parse_sql(sql_write).unwrap() {
        sql::SqlRequest::Write(reqs) => {
            tx_mgr.execute_smart(reqs).await.unwrap();
        }
        _ => panic!("Should be Write"),
    }

    // 6. VERIFICATION INDEX (QueryEngine doit trouver Charlie via l'index)
    println!("--- Step 5: Verify Index Consistency ---");
    let engine = QueryEngine::new(&col_mgr);

    // On utilise l'index BTree sur l'age
    let q_btree = raise::json_db::query::Query {
        collection: "users".into(),
        filter: Some(raise::json_db::query::QueryFilter {
            operator: raise::json_db::query::FilterOperator::And,
            conditions: vec![raise::json_db::query::Condition::eq("age", json!(25))],
        }),
        sort: None,
        limit: None,
        offset: None,
        projection: None,
    };

    // Le QueryEngine utilise l'index. Si l'index n'est pas à jour, il ne trouvera rien.
    let res_btree = engine.execute_query(q_btree).await.unwrap();
    assert_eq!(res_btree.documents.len(), 1);
    assert_eq!(res_btree.documents[0]["name"], "Charlie");

    println!("✅ GLOBAL INTEGRATION SUCCESS");
}
