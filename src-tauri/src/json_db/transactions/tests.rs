use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::file_storage;
use crate::json_db::test_utils::init_test_env;
use crate::json_db::transactions::TransactionManager;
use serde_json::json;

// Helper pour initialiser une collection avec un schéma valide
fn setup_collection(cm: &CollectionsManager, name: &str) {
    // On utilise un schéma existant dans le projet (ex: actors/actor.schema.json)
    // car le test_utils::init_test_env copie les schémas réels.
    // Si vous n'avez pas ce schéma, il faut en créer un dummy.
    // Supposons que 'actors/actor.schema.json' existe.

    // Note : Dans le vrai code, create_collection sans argument utilise "unknown".
    // Ici on contourne en appelant la version bas niveau ou en créant le fichier "unknown".

    // Option la plus robuste pour le test : créer un fichier schéma minimal
    let schema_path = cm
        .cfg
        .db_schemas_root(&cm.space, &cm.db)
        .join("minimal.json");
    std::fs::write(&schema_path, r#"{"type":"object"}"#).expect("write dummy schema");

    // Maintenant on peut créer la collection liée à ce schéma
    file_storage::create_collection(cm.cfg, &cm.space, &cm.db, name, "minimal.json")
        .expect("create collection with schema");

    // On force aussi la création des index (normalement fait par le manager)
    crate::json_db::indexes::create_collection_indexes(
        cm.cfg,
        &cm.space,
        &cm.db,
        name,
        "minimal.json",
    )
    .expect("create indexes");
}

#[test]
fn test_transaction_commit_success() {
    let env = init_test_env();
    let cfg = &env.cfg;
    let space = &env.space;
    let db = &env.db;

    file_storage::create_db(cfg, space, db).expect("create db");

    let cm = CollectionsManager::new(cfg, space, db);
    setup_collection(&cm, "users"); // <--- Utilisation du helper

    let tm = TransactionManager::new(cfg, space, db);

    let result = tm.execute(|tx| {
        tx.add_insert(
            "users",
            "user-1",
            json!({"id": "user-1", "name": "Alice", "balance": 100}),
        );

        tx.add_insert(
            "users",
            "user-2",
            json!({"id": "user-2", "name": "Bob", "balance": 50}),
        );

        Ok(())
    });

    assert!(
        result.is_ok(),
        "La transaction aurait dû réussir : {:?}",
        result.err()
    );

    // Vérifications
    let alice = cm.get("users", "user-1").expect("Alice doit exister");
    assert_eq!(alice["name"], "Alice");

    let index = file_storage::read_index(cfg, space, db).expect("read index");
    let collection_idx = index
        .collections
        .get("users")
        .expect("collection users index");
    assert!(collection_idx.items.iter().any(|i| i.file == "user-1.json"));
}

#[test]
fn test_transaction_rollback_on_error() {
    let env = init_test_env();
    let cfg = &env.cfg;
    let space = &env.space;
    let db = &env.db;

    file_storage::create_db(cfg, space, db).expect("create db");

    let cm = CollectionsManager::new(cfg, space, db);
    setup_collection(&cm, "users"); // <--- Utilisation du helper

    // État initial
    let tm = TransactionManager::new(cfg, space, db);
    tm.execute(|tx| {
        tx.add_insert(
            "users",
            "user-1",
            json!({"id": "user-1", "name": "Alice", "balance": 100}),
        );
        Ok(())
    })
    .expect("Setup initial failed");

    // Transaction qui échoue
    let result = tm.execute(|tx| {
        tx.add_update(
            "users",
            "user-1",
            None,
            json!({"id": "user-1", "name": "Alice", "balance": 0}),
        );
        tx.add_insert(
            "users",
            "user-3",
            json!({"id": "user-3", "name": "Charlie"}),
        );
        anyhow::bail!("Solde insuffisant !")
    });

    assert!(result.is_err());

    let alice = cm.get("users", "user-1").unwrap();
    assert_eq!(alice["balance"], 100, "Rollback réussi");
    assert!(
        cm.get("users", "user-3").is_err(),
        "Charlie ne doit pas exister"
    );
}

#[test]
fn test_wal_persistence() {
    let env = init_test_env();

    file_storage::create_db(&env.cfg, &env.space, &env.db).expect("create db");

    let tm = TransactionManager::new(&env.cfg, &env.space, &env.db);
    let cm = CollectionsManager::new(&env.cfg, &env.space, &env.db);
    setup_collection(&cm, "logs"); // <--- Utilisation du helper

    tm.execute(|tx| {
        tx.add_insert(
            "logs",
            "log-1",
            json!({
                "id": "log-1",
                "msg": "test"
            }),
        );
        Ok(())
    })
    .unwrap();

    let wal_path = env.cfg.db_root(&env.space, &env.db).join("_wal.jsonl");
    let content = std::fs::read_to_string(wal_path).expect("lecture wal");

    assert!(content.contains("log-1"));
    assert!(content.contains("committed"));
}
