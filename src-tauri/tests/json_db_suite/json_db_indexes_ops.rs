// FICHIER : src-tauri/tests/json_db_suite/json_db_indexes_ops.rs

use crate::{ensure_db_exists, init_test_env}; // Imports nettoyés
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::StorageEngine;
use serde_json::json;
use std::fs;

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_create_and_drop_index_lifecycle() {
    // CORRECTION E0277 : Ces helpers sont synchrones dans cette suite, pas de .await ici
    let env = init_test_env().await;
    ensure_db_exists(&env.cfg, &env.space, &env.db);

    let storage = StorageEngine::new(env.cfg.clone());
    let mgr = CollectionsManager::new(&storage, &env.space, &env.db);

    let collection = "indexed_articles";

    // CORRECTION E0599 : create_collection est désormais asynchrone
    mgr.create_collection(collection, None)
        .await
        .expect("create_collection failed");

    // 1. Insertion de données (pour vérifier que l'index se remplit à la création)
    let doc = json!({
        "handle": "test-handle",
        "slug": "test-handle",
        "displayName": "Test Item",
        "title": "Test Title",
        "status": "draft"
    });

    // CORRECTION E0599 : insert_with_schema est désormais asynchrone
    mgr.insert_with_schema(collection, doc)
        .await
        .expect("insert failed");

    // 2. Création de l'Index (Hash sur 'handle')
    println!("🏗️ Création de l'index...");
    // CORRECTION E0599 : Les opérations d'indexation sont passées en asynchrone
    mgr.create_index(collection, "handle", "hash")
        .await
        .expect("create_index failed");

    // VÉRIFICATION 1 : _meta.json mis à jour
    let meta_path = env
        .cfg
        .db_collection_path(&env.space, &env.db, collection)
        .join("_meta.json");
    let meta_content = fs::read_to_string(&meta_path).expect("Lecture _meta.json impossible");

    assert!(
        meta_content.contains("\"name\": \"handle\""),
        "_meta.json doit contenir la définition de l'index"
    );
    assert!(
        meta_content.contains("\"index_type\": \"hash\""),
        "_meta.json doit spécifier le type hash"
    );

    // VÉRIFICATION 2 : Fichier physique créé
    let index_path = env
        .cfg
        .db_collection_path(&env.space, &env.db, collection)
        .join("_indexes")
        .join("handle.hash.idx");

    assert!(
        index_path.exists(),
        "Le fichier physique de l'index doit exister"
    );

    // 3. Suppression de l'Index
    println!("🔥 Suppression de l'index...");
    // CORRECTION E0599 : drop_index nécessite également .await
    mgr.drop_index(collection, "handle")
        .await
        .expect("drop_index failed");

    // VÉRIFICATION 3 : _meta.json nettoyé
    let meta_content_after = fs::read_to_string(&meta_path).unwrap();
    assert!(
        !meta_content_after.contains("\"name\": \"handle\""),
        "L'index ne doit plus apparaître dans _meta.json"
    );

    // VÉRIFICATION 4 : Fichier physique supprimé
    assert!(
        !index_path.exists(),
        "Le fichier physique de l'index doit avoir été supprimé"
    );
}
