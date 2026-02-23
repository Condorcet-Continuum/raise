// FICHIER : src-tauri/tests/json_db_suite/integration_suite.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::{
    collections::manager::CollectionsManager,
    indexes::manager::IndexManager,
    query::{sql, Condition, FilterOperator, Query, QueryEngine, QueryFilter},
    storage::JsonDbConfig,
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::utils::{prelude::*, Arc}; // SSOT : Apporte json!, Arc, Value, etc.

#[tokio::test]
async fn test_json_db_global_scenario() {
    // 1. SETUP ENVIRONNEMENT (Robuste & Isolé)
    let env = setup_test_env(LlmMode::Disabled).await;

    // Le TransactionManager a besoin de la configuration enveloppée dans un Arc
    let config = Arc::new(JsonDbConfig {
        data_root: env.domain_path.clone(),
    });

    let col_mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let mut idx_mgr = IndexManager::new(&env.storage, &env.space, &env.db);
    let tx_mgr = TransactionManager::new(&config, &env.space, &env.db);

    // 2. CRÉATION SCHÉMA & INDEX
    println!("--- Step 1: Create Collection & Index ---");
    // Note : Plus besoin de init_db(), setup_test_env s'en est déjà chargé !

    col_mgr
        .create_collection("users", None)
        .await
        .expect("❌ Échec de la création de la collection 'users'");

    idx_mgr
        .create_index("users", "email", "hash")
        .await
        .expect("❌ Échec de la création de l'index sur 'email'");

    idx_mgr
        .create_index("users", "age", "btree")
        .await
        .expect("❌ Échec de la création de l'index sur 'age'");

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

    tx_mgr
        .execute_smart(tx_reqs)
        .await
        .expect("❌ Échec de la transaction d'insertion initiale");

    // 4. VERIFICATION VIA SQL (SELECT)
    println!("--- Step 3: Query Data (SQL SELECT) ---");
    let sql_read = "SELECT name, age FROM users WHERE email = 'alice@corp.com'";

    let query = match sql::parse_sql(sql_read).expect("❌ Erreur de parsing du SQL SELECT") {
        sql::SqlRequest::Read(q) => q,
        _ => panic!("❌ La requête SQL devrait être de type Read"),
    };

    let engine = QueryEngine::new(&col_mgr);
    let res = engine
        .execute_query(query)
        .await
        .expect("❌ Échec de l'exécution du SELECT");

    assert_eq!(
        res.documents.len(),
        1,
        "Devrait trouver exactement 1 document pour Alice"
    );
    assert_eq!(
        res.documents[0]["name"], "Alice",
        "Le nom devrait être Alice"
    );
    assert!(
        res.documents[0].get("email").is_none(),
        "L'email ne devrait pas être projeté (SELECT name, age)"
    );

    // 5. MODIFICATION VIA SQL (INSERT)
    println!("--- Step 4: Write Data (SQL INSERT) ---");
    let sql_write =
        "INSERT INTO users (name, email, age) VALUES ('Charlie', 'charlie@corp.com', 25)";

    match sql::parse_sql(sql_write).expect("❌ Erreur de parsing du SQL INSERT") {
        sql::SqlRequest::Write(reqs) => {
            tx_mgr
                .execute_smart(reqs)
                .await
                .expect("❌ Échec de l'insertion SQL de Charlie");
        }
        _ => panic!("❌ La requête SQL devrait être de type Write"),
    }

    // 6. VERIFICATION INDEX
    println!("--- Step 5: Verify Index Consistency ---");

    let q_btree = Query {
        collection: "users".into(),
        filter: Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq("age", json!(25))],
        }),
        sort: None,
        limit: None,
        offset: None,
        projection: None,
    };

    let res_btree = engine
        .execute_query(q_btree)
        .await
        .expect("❌ Échec de la requête sur l'index BTree");

    assert_eq!(
        res_btree.documents.len(),
        1,
        "L'index devrait trouver exactement 1 document via l'âge"
    );
    assert_eq!(
        res_btree.documents[0]["name"], "Charlie",
        "Le document trouvé devrait être Charlie"
    );

    println!("✅ GLOBAL INTEGRATION SUCCESS");
}
