// FICHIER : src-tauri/tests/json_db_suite/integration_suite.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::{
    collections::manager::CollectionsManager,
    indexes::manager::IndexManager,
    query::{sql, Condition, FilterOperator, Query, QueryEngine, QueryFilter},
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::utils::prelude::*; // SSOT : Apporte json!, SharedRef, JsonValue, RaiseResult, etc.

#[async_test]
async fn test_json_db_global_scenario() {
    // 🎯 PATTERN ZERO DETTE : Fonction interne pour propager les erreurs proprement avec `?`
    async fn run() -> RaiseResult<()> {
        // 1. SETUP ENVIRONNEMENT (Robuste & Isolé)
        let env = setup_test_env(LlmMode::Disabled).await;
        let col_mgr = CollectionsManager::new(&env.sandbox.storage, &env.space, &env.db);
        let mut idx_mgr = IndexManager::new(&env.sandbox.storage, &env.space, &env.db);
        let tx_mgr = TransactionManager::new(&env.sandbox.storage, &env.space, &env.db);

        // 2. CRÉATION SCHÉMA & INDEX
        println!("--- Step 1: Create Collection & Index ---");

        col_mgr
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        idx_mgr.create_index("users", "email", "hash").await?;
        idx_mgr.create_index("users", "age", "btree").await?;

        // 3. INSERTION VIA TRANSACTION (API Smart)
        println!("--- Step 2: Insert Data (Transaction) ---");
        let tx_reqs = vec![
            TransactionRequest::Insert {
                collection: "users".into(),
                id: Some("u1".into()),
                document: json_value!({ "name": "Alice", "email": "alice@corp.com", "age": 30 }),
            },
            TransactionRequest::Insert {
                collection: "users".into(),
                id: Some("u2".into()),
                document: json_value!({ "name": "Bob", "email": "bob@corp.com", "age": 40 }),
            },
        ];

        tx_mgr.execute_smart(tx_reqs).await?;

        // 4. VERIFICATION VIA SQL (SELECT)
        println!("--- Step 3: Query Data (SQL SELECT) ---");
        let sql_read = "SELECT name, age FROM users WHERE email = 'alice@corp.com'";

        let query = match sql::parse_sql(sql_read)? {
            sql::SqlRequest::Read(q) => q,
            _ => {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "La requête SQL devrait être de type Read"
                );
            }
        };

        let engine = QueryEngine::new(&col_mgr);
        let res = engine.execute_query(query).await?;

        if res.documents.len() != 1 {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Devrait trouver exactement 1 document pour Alice"
            );
        }
        if res.documents[0]["name"] != "Alice" {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Le nom devrait être Alice"
            );
        }
        if res.documents[0].get("email").is_some() {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "L'email ne devrait pas être projeté (SELECT name, age)"
            );
        }

        // 5. MODIFICATION VIA SQL (INSERT)
        println!("--- Step 4: Write Data (SQL INSERT) ---");
        let sql_write =
            "INSERT INTO users (name, email, age) VALUES ('Charlie', 'charlie@corp.com', 25)";

        match sql::parse_sql(sql_write)? {
            sql::SqlRequest::Write(reqs) => {
                tx_mgr.execute_smart(reqs).await?;
            }
            _ => {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "La requête SQL devrait être de type Write"
                );
            }
        }

        // 6. VERIFICATION INDEX
        println!("--- Step 5: Verify Index Consistency ---");

        let q_btree = Query {
            collection: "users".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("age", json_value!(25))],
            }),
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        };

        let res_btree = engine.execute_query(q_btree).await?;

        if res_btree.documents.len() != 1 {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "L'index devrait trouver exactement 1 document via l'âge"
            );
        }
        if res_btree.documents[0]["name"] != "Charlie" {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Le document trouvé devrait être Charlie"
            );
        }

        println!("✅ GLOBAL INTEGRATION SUCCESS");
        Ok(())
    }

    // Interception au niveau du Test Runner
    if let Err(e) = run().await {
        panic!(
            "❌ Échec du test d'intégration 'test_json_db_global_scenario' : {}",
            e
        );
    }
}
