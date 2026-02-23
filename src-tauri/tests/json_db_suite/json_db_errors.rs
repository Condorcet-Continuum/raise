// FICHIER : src-tauri/tests/json_db_suite/json_db_errors.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::storage::file_storage::{create_db, open_db};
use raise::json_db::storage::JsonDbConfig;

#[tokio::test]
async fn open_missing_db_fails() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env(LlmMode::Disabled).await;

    // 2. Création de la config de stockage pointant vers notre dossier isolé
    let cfg = JsonDbConfig {
        data_root: env.domain_path.clone(),
    };

    let db_missing = "db_introuvable_123";

    // 3. Tentative d'ouverture d'une DB inexistante
    let res = open_db(&cfg, &env.space, db_missing).await;

    assert!(
        res.is_err(),
        "❌ open_db devrait échouer si la base de données '{}' n'existe pas, mais l'opération a réussi de manière inattendue.",
        db_missing
    );
}

#[tokio::test]
async fn create_db_is_idempotent() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env(LlmMode::Disabled).await;

    let cfg = JsonDbConfig {
        data_root: env.domain_path.clone(),
    };

    // Note : setup_test_env() a déjà appelé init_db() en coulisses,
    // donc le dossier de la DB existe déjà ! Cela rend ce test d'idempotence encore plus pertinent.

    // 2. Premier appel explicite à create_db (doit réussir même si le dossier est déjà là)
    create_db(&cfg, &env.space, &env.db)
        .await
        .expect("❌ Le premier appel à create_db doit réussir");

    // 3. Second appel à create_db (Vérification stricte de l'idempotence)
    let res = create_db(&cfg, &env.space, &env.db).await;

    assert!(
        res.is_ok(),
        "❌ Le second create_db devrait réussir (comportement idempotent), mais a échoué avec l'erreur : {:?}",
        res.err()
    );
}
