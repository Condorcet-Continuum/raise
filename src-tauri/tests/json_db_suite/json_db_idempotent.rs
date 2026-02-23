// FICHIER : src-tauri/tests/json_db_suite/json_db_idempotent.rs

use crate::common::{setup_test_env, LlmMode};
use raise::json_db::storage::file_storage::{create_db, drop_db, open_db, DropMode};
use raise::json_db::storage::JsonDbConfig;

#[tokio::test]
async fn drop_is_idempotent_and_recreate_works() {
    // 1. Initialisation de l'environnement isolé
    let env = setup_test_env(LlmMode::Disabled).await;

    // On recrée la config à partir du dossier isolé
    let cfg = JsonDbConfig {
        data_root: env.domain_path.clone(),
    };

    let space = &env.space;
    // 💡 ASTUCE : On utilise un nom de DB vierge pour s'assurer qu'elle n'existe pas au début du test
    let db = "test_idempotence_db";

    // --- ÉTAPE 1 : Drop sur DB inexistante (Idempotence) ---
    println!("--- Step 1: Testing Drop Idempotency ---");

    drop_db(&cfg, space, db, DropMode::Soft)
        .await
        .expect("❌ Le Soft Drop sur une DB inexistante devrait réussir (comportement idempotent)");

    drop_db(&cfg, space, db, DropMode::Hard)
        .await
        .expect("❌ Le Hard Drop sur une DB inexistante devrait réussir (comportement idempotent)");

    // --- ÉTAPE 2 : Cycle de vie (Create -> Open -> Hard Drop) ---
    println!("--- Step 2: Testing Full Lifecycle ---");

    create_db(&cfg, space, db)
        .await
        .expect("❌ La création de la nouvelle base de données doit réussir");

    let db_root = cfg.db_root(space, db);

    // Vérification physique
    assert!(
        db_root.exists(),
        "❌ Le dossier racine de la DB doit exister physiquement après create_db"
    );

    // Vérification logique
    open_db(&cfg, space, db)
        .await
        .expect("❌ L'ouverture (open_db) doit réussir sur une DB qui vient d'être créée");

    // Suppression définitive
    drop_db(&cfg, space, db, DropMode::Hard)
        .await
        .expect("❌ Le Hard Drop final doit réussir pour clôturer le cycle");

    // Vérification finale
    assert!(
        !db_root.exists(),
        "❌ Le dossier racine doit avoir totalement disparu après le Hard Drop"
    );

    println!("✅ LIFECYCLE & IDEMPOTENCY SUCCESS");
}
