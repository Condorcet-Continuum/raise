// src-tauri/tests/json_db_suite/json_db_errors.rs

use crate::{init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::storage::file_storage::{create_db, open_db};

#[test]
fn open_missing_db_fails() {
    let env = init_test_env();
    let cfg = &env.cfg;
    let space = TEST_SPACE;
    let db_missing = "db_introuvable_123";

    // 1. open sur DB inexistante → Err
    assert!(
        open_db(cfg, space, db_missing).is_err(),
        "open_db devrait échouer si la DB n'existe pas"
    );
}

#[test]
fn create_db_is_idempotent() {
    let env = init_test_env();
    let cfg = &env.cfg;
    let space = TEST_SPACE;
    let db = TEST_DB;

    // 1. Premier create_db → OK
    create_db(cfg, space, db).expect("premier create_db doit réussir");

    // 2. Second create_db → OK (Idempotent)
    // Modification de la logique de test : il est préférable que create_db soit idempotent
    // pour éviter des erreurs dans les scripts de déploiement ou de redémarrage.
    let res = create_db(cfg, space, db);
    assert!(
        res.is_ok(),
        "second create_db devrait réussir (idempotence), erreur reçue : {:?}",
        res.err()
    );
}
