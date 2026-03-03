// FICHIER : src-tauri/tests/json_db_suite/json_db_errors.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::storage::file_storage::{create_db, open_db};
use raise::json_db::storage::JsonDbConfig;
use raise::utils::json::json; // 🎯 AJOUT VITAL pour le 4ème argument

#[tokio::test]
async fn open_missing_db_fails() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = JsonDbConfig {
        data_root: env.domain_path.clone(),
    };

    let db_missing = "db_introuvable_123";
    let res = open_db(&cfg, &env.space, db_missing).await;

    assert!(
        res.is_err(),
        "❌ open_db devrait échouer si la base de données '{}' n'existe pas.",
        db_missing
    );
}

#[tokio::test]
async fn create_db_is_idempotent() {
    let env = setup_test_env(LlmMode::Disabled).await;
    let cfg = JsonDbConfig {
        data_root: env.domain_path.clone(),
    };

    // 🎯 On fournit un plan de construction vide (mais valide syntaxiquement)
    let dummy_doc = json!({ "collections": {}, "rules": {}, "schemas": {} });

    // 1. Premier appel : la DB existe DÉJÀ (créée par setup_test_env).
    // Le "Return Early" de l'idempotence doit renvoyer false.
    let created = create_db(&cfg, &env.space, &env.db, &dummy_doc)
        .await
        .expect("❌ L'appel à create_db a échoué");

    assert!(
        !created,
        "La base existait déjà, create_db aurait dû retourner false"
    );

    // 2. Second appel : Toujours idempotent
    let res = create_db(&cfg, &env.space, &env.db, &dummy_doc).await;

    assert!(
        res.is_ok(),
        "❌ Le second create_db devrait réussir, mais a échoué avec : {:?}",
        res.err()
    );
    assert!(!res.unwrap(), "Le second appel doit aussi retourner false");
}
