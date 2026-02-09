// FICHIER : src-tauri/tests/json_db_suite/json_db_lifecycle.rs

use crate::{init_test_env, TEST_DB, TEST_SPACE};
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::file_storage::{create_db, drop_db, open_db, DropMode};
use raise::utils::{
    fs,
    json::{self, json, Value},
};
// -----------------------

#[tokio::test]
async fn db_lifecycle_minimal() {
    let env = init_test_env().await;
    let cfg = &env.cfg;
    let space = TEST_SPACE;
    let db = TEST_DB;

    // CREATE
    // CORRECTION E0599 : create_db est asynchrone, ajout de .await
    create_db(cfg, space, db)
        .await
        .expect("create_db doit réussir");

    let db_root = cfg.db_root(space, db);
    assert!(db_root.is_dir(), "db root doit exister physiquement");

    let _index_path = cfg.db_root(space, db).join("_system.json");

    let schemas_path = cfg.db_schemas_root(space, db);
    assert!(schemas_path.exists(), "le dossier schemas doit exister");

    // OPEN
    open_db(cfg, space, db).await.expect("open_db doit réussir");

    // DROP (Soft)
    // CORRECTION E0599 : drop_db est asynchrone, ajout de .await
    drop_db(cfg, space, db, DropMode::Soft)
        .await
        .expect("drop_db soft doit réussir");
    assert!(
        !db_root.exists(),
        "après soft drop, le dossier original ne doit plus exister"
    );

    // Vérifie qu’un dossier renommé existe
    let mut found_soft = false;
    let space_root = cfg.data_root.join(space);
    let mut entries = fs::read_dir(&space_root).await.expect("ls space_root");
    while let Some(entry) = entries.next_entry().await.expect("entry") {
        let p = entry.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        // Vérification dossier renommé
        if name.starts_with(db) && name.contains(".deleted-") && p.is_dir() {
            found_soft = true;
            break;
        }
    }
    assert!(
        found_soft,
        "le dossier renommé *.deleted-<ts> doit exister après un soft drop"
    );

    // Re-crée puis DROP (Hard)
    create_db(cfg, space, db)
        .await
        .expect("recreate_db doit réussir");
    assert!(db_root.exists());

    drop_db(cfg, space, db, DropMode::Hard)
        .await
        .expect("drop_db hard doit réussir");

    assert!(
        !db_root.exists(),
        "après hard drop, la DB doit être supprimée définitivement"
    );
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn db_lifecycle_create_open_drop() {
    let test_env = init_test_env().await;
    let cfg = &test_env.cfg;
    let space = "un2";
    let db = "_system_lifecycle_test";

    // Nettoyage manuel au cas où
    let root = cfg.db_root(space, db);
    if fs::exists(&root).await {
        fs::remove_dir_all(&root).await.unwrap();
    }
    // 1. Création
    create_db(cfg, space, db).await.expect("create");

    // 2. Ouverture (Sync)
    open_db(cfg, space, db).await.expect("open");

    // 3. Soft drop
    drop_db(cfg, space, db, DropMode::Soft)
        .await
        .expect("soft drop");

    // 4. Hard drop
    drop_db(cfg, space, db, DropMode::Hard)
        .await
        .expect("hard drop");
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_collection_drop_cleans_system_index() {
    let env = init_test_env().await;
    let cfg = &env.cfg;
    let space = TEST_SPACE;
    let db = TEST_DB;

    // On utilise le manager importé correctement depuis 'raise'
    let mgr = CollectionsManager::new(&env.storage, space, db);
    let collection = "temp_collection_to_drop";

    // 1. Création de la collection
    // CORRECTION E0599 : create_collection est asynchrone
    mgr.create_collection(collection, None)
        .await
        .expect("create collection failed");

    // 2. Vérification : Elle doit exister physiquement
    let col_path = cfg.db_collection_path(space, db, collection);
    assert!(col_path.exists(), "Le dossier collection doit exister");

    // 3. Vérification : Elle doit être dans _system.json
    let sys_path = cfg.db_root(space, db).join("_system.json");
    let content_after = fs::read_to_string(&sys_path)
        .await
        .expect("read _system.json");

    // CORRECTION : Parsing via utils::json
    let sys_json: Value = json::parse(&content_after).expect("parse");
    assert!(
        sys_json
            .pointer(&format!("/collections/{}", collection))
            .is_some(),
        "La collection doit être présente dans _system.json avant suppression"
    );

    // 4. Suppression (Drop)
    mgr.drop_collection(collection)
        .await
        .expect("drop collection failed");

    // 5. Vérification : Elle ne doit plus exister physiquement
    assert!(
        !col_path.exists(),
        "Le dossier collection doit avoir disparu"
    );

    // 6. Vérification CRITIQUE : Elle ne doit plus être dans _system.json
    let content_after = fs::read_to_string(&sys_path)
        .await
        .expect("read _system.json");
    let sys_json_after: Value = json::parse(&content_after).expect("parse");

    assert!(
        sys_json_after
            .pointer(&format!("/collections/{}", collection))
            .is_none(),
        "La collection DOIT être retirée de _system.json après suppression"
    );
}

#[tokio::test] // CORRECTION : Passage en test asynchrone
async fn test_system_index_strict_conformance() {
    // 1. Initialisation (Sync)
    let env = init_test_env().await;
    let cfg = &env.cfg;
    let space = TEST_SPACE;
    let db = TEST_DB;

    // --- DIAGNOSTIC DU SCHÉMA COPIÉ ---
    let schema_path = cfg
        .db_schemas_root(space, db)
        .join("v1/db/index.schema.json");

    assert!(
        schema_path.exists(),
        "❌ Le fichier index.schema.json n'a pas été copié !"
    );

    let schema_content = fs::read_to_string(&schema_path)
        .await
        .expect("Lecture schéma");

    if !schema_content.contains("base.schema.json") {
        println!("🔥 CONTENU DU SCHÉMA INCORRECT :\n{}", schema_content);
        panic!("❌ Le fichier index.schema.json copié est OBSOLÈTE ! Il manque le 'allOf' vers base.schema.json.");
    }
    // ----------------------------------

    // 2. Lecture du fichier généré
    let sys_path = cfg.db_root(space, db).join("_system.json");
    assert!(
        sys_path.exists(),
        "Le fichier _system.json doit exister physiquement"
    );

    let content = fs::read_to_string(&sys_path)
        .await
        .expect("Lecture _system.json");
    let doc: Value = serde_json::from_str(&content).expect("JSON malformé");

    // 3. Vérifications strictes
    if doc.get("id").is_none() {
        println!(
            "📄 Contenu de _system.json généré :\n{}",
            serde_json::to_string_pretty(&doc).unwrap()
        );
        panic!("❌ L'index système N'A PAS d'ID.");
    }

    assert!(doc.get("createdAt").is_some(), "Manque createdAt");

    let expected_schema = format!("db://{}/{}/schemas/v1/db/index.schema.json", space, db);
    assert_eq!(doc.get("$schema"), Some(&json!(expected_schema)));

    // 4. Validation finale
    let registry = SchemaRegistry::from_db(cfg, space, db)
        .await
        .expect("Chargement registre");
    let validator = SchemaValidator::compile_with_registry(&expected_schema, &registry)
        .expect("Compilation validateur");

    if let Err(e) = validator.validate(&doc) {
        panic!("🚨 Validation finale échouée : {}", e);
    }
}
