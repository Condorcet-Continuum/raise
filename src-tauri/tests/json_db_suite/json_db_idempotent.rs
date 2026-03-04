// FICHIER : src-tauri/tests/json_db_suite/json_db_idempotent.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::storage::file_storage::{create_db, drop_db, open_db, DropMode};
use raise::json_db::storage::JsonDbConfig;
use raise::utils::json::json; // 🎯 Requis pour le plan de construction

#[tokio::test]
async fn drop_is_idempotent_and_recreate_works() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env(LlmMode::Disabled).await;

    let cfg = JsonDbConfig {
        data_root: env.sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap(),
    };

    let space = &env.space;
    let db = "test_idempotence_db";

    // 🎯 On définit un document d'index minimal mais valide pour l'introspection
    let system_doc = json!({
        "collections": { "test_col": { "items": [] } },
        "rules": {},
        "schemas": { "v1": {} }
    });

    // --- ÉTAPE 1 : Drop sur DB inexistante (Idempotence) ---
    // Aucun changement ici, drop_db ne nécessite pas le system_doc
    drop_db(&cfg, space, db, DropMode::Soft)
        .await
        .expect("❌ Le Soft Drop sur une DB inexistante devrait réussir");

    drop_db(&cfg, space, db, DropMode::Hard)
        .await
        .expect("❌ Le Hard Drop sur une DB inexistante devrait réussir");

    // --- ÉTAPE 2 : Cycle de vie (Create -> Open -> Hard Drop) ---

    // 🎯 FIX : Passage du 4ème argument (system_doc) pour l'introspection dynamique
    create_db(&cfg, space, db, &system_doc)
        .await
        .expect("❌ La création de la nouvelle base de données doit réussir");

    let db_root = cfg.db_root(space, db);

    // Vérification physique de la racine
    assert!(
        db_root.exists(),
        "❌ Le dossier racine de la DB doit exister"
    );

    // 🎯 VERIFICATION DE L'INTROSPECTION :
    // On vérifie que create_db a bien créé le sous-dossier défini dans le JSON
    assert!(
        db_root.join("collections/test_col").exists(),
        "❌ L'introspection dynamique aurait dû créer le dossier 'collections/test_col'"
    );

    // Vérification logique
    open_db(&cfg, space, db)
        .await
        .expect("❌ L'ouverture (open_db) doit réussir sur une DB créée");

    // Suppression définitive
    drop_db(&cfg, space, db, DropMode::Hard)
        .await
        .expect("❌ Le Hard Drop final doit réussir");

    // Vérification finale
    assert!(
        !db_root.exists(),
        "❌ Le dossier racine doit avoir disparu après le Hard Drop"
    );
}
