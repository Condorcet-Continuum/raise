// FICHIER : src-tauri/tests/json_db_suite/json_db_indexes_ops.rs

use crate::common::setup_test_env; // Notre socle SSOT
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::io;
use raise::utils::prelude::*;

#[tokio::test]
async fn test_create_and_drop_index_lifecycle() {
    // 1. SETUP ENVIRONNEMENT (Isolé et unifié)
    let env = setup_test_env().await;

    // Le CollectionsManager est déjà initialisé dans UnifiedTestEnv,
    // mais on en crée une instance locale pour plus de clarté dans le test.
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let collection = "indexed_articles";

    // 2. PRÉPARATION : Création collection et données
    mgr.create_collection(collection, None)
        .await
        .expect("❌ Impossible de créer la collection de test");

    let doc = json!({
        "handle": "test-handle",
        "slug": "test-handle",
        "displayName": "Test Item",
        "status": "draft"
    });

    mgr.insert_with_schema(collection, doc)
        .await
        .expect("❌ L'insertion initiale avant indexation a échoué");

    // 3. OPÉRATION : Création de l'Index (Hash sur 'handle')
    println!("🏗️ Création de l'index...");
    mgr.create_index(collection, "handle", "hash")
        .await
        .expect("❌ La création de l'index hash sur 'handle' a échoué");

    // 4. VÉRIFICATION PHYSIQUE 1 : Le fichier _meta.json
    let meta_path = env
        .storage
        .config
        .db_collection_path(&env.space, &env.db, collection)
        .join("_meta.json");

    let meta_content = io::read_to_string(&meta_path)
        .await
        .expect("❌ Lecture du fichier _meta.json impossible après indexation");

    assert!(
        meta_content.contains("\"name\": \"handle\""),
        "ERREUR : _meta.json ne contient pas la définition de l'index 'handle'"
    );

    // 5. VÉRIFICATION PHYSIQUE 2 : Le fichier d'index .idx
    let index_path = env
        .storage
        .config
        .db_collection_path(&env.space, &env.db, collection)
        .join("_indexes")
        .join("handle.hash.idx");

    assert!(
        io::exists(&index_path).await,
        "ERREUR : Le fichier physique de l'index ({:?}) est introuvable sur le disque",
        index_path
    );

    // 6. OPÉRATION : Suppression de l'Index
    println!("🔥 Suppression de l'index...");
    mgr.drop_index(collection, "handle")
        .await
        .expect("❌ La suppression de l'index (drop_index) a échoué");

    // 7. VÉRIFICATION FINALE : Nettoyage
    let meta_content_after = io::read_to_string(&meta_path)
        .await
        .expect("❌ Lecture du fichier _meta.json impossible après suppression");

    assert!(
        !meta_content_after.contains("\"name\": \"handle\""),
        "ERREUR : L'index 'handle' est toujours présent dans _meta.json après suppression"
    );

    assert!(
        !io::exists(&index_path).await,
        "ERREUR : Le fichier physique de l'index n'a pas été supprimé du disque"
    );

    println!("✅ INDEX LIFECYCLE SUCCESS");
}
