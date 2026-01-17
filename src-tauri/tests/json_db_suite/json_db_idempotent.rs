// FICHIER : src-tauri/tests/json_db_suite/json_db_idempotent.rs

use crate::{init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::storage::file_storage::{create_db, drop_db, open_db, DropMode};

#[tokio::test]
async fn drop_is_idempotent_and_recreate_works() {
    // init_test_env() est synchrone dans cette suite de tests
    let test_env = init_test_env().await;
    let cfg = &test_env.cfg;

    let space = TEST_SPACE;
    let db = TEST_DB;

    // 1) Drop sur DB inexistante → OK (idempotent)
    // CORRECTION : drop_db est asynchrone, ajout de .await
    drop_db(cfg, space, db, DropMode::Soft)
        .await
        .expect("soft drop sur DB inexistante devrait réussir");

    drop_db(cfg, space, db, DropMode::Hard)
        .await
        .expect("hard drop sur DB inexistante devrait réussir");

    // 2) Cycle de vie : create → open → hard drop
    // CORRECTION : create_db est asynchrone, ajout de .await
    create_db(cfg, space, db)
        .await
        .expect("create doit réussir");

    let db_root = cfg.db_root(space, db);

    // Vérification physique
    assert!(
        db_root.exists(),
        "Le dossier racine de la DB doit exister après create"
    );

    // Vérification logique
    // CORRECTION : open_db est synchrone, pas de .await ici
    open_db(cfg, space, db).expect("open doit réussir sur une DB existante");

    // Suppression
    // CORRECTION : .await ajouté pour la suppression finale
    drop_db(cfg, space, db, DropMode::Hard)
        .await
        .expect("hard drop final doit réussir");

    // Vérification finale
    assert!(!db_root.exists(), "Le dossier racine doit avoir disparu");
}
