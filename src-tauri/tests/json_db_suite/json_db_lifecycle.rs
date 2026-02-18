// FICHIER : src-tauri/tests/json_db_suite/json_db_lifecycle.rs

use crate::common::setup_test_env;
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::schema::{SchemaRegistry, SchemaValidator};
use raise::json_db::storage::file_storage::{create_db, drop_db, open_db, DropMode};
use raise::json_db::storage::JsonDbConfig;
use raise::utils::io;
use raise::utils::prelude::*;

#[tokio::test]
async fn db_lifecycle_minimal() {
    // 1. Initialisation de l'environnement isolé (UUID unique par thread)
    let env = setup_test_env().await;
    let cfg = JsonDbConfig {
        data_root: env.domain_path.clone(),
    };

    // On utilise un nom de DB spécifique pour ce test de cycle de vie
    let space = "lifecycle_minimal";
    let db = "test_db";

    // --- CREATE ---
    create_db(&cfg, space, db)
        .await
        .expect("❌ create_db doit réussir");

    let db_root = cfg.db_root(space, db);
    assert!(
        db_root.is_dir(),
        "❌ Le dossier racine de la DB doit exister physiquement"
    );

    // ✅ CORRECTION : On retire l'assertion sur le dossier schemas
    // Avec l'architecture "Zéro Copie", create_db ne crée plus ce dossier.
    // let schemas_path = cfg.db_schemas_root(space, db);
    // assert!(schemas_path.exists(), "❌ Le dossier schemas doit avoir été créé");

    // --- OPEN ---
    open_db(&cfg, space, db)
        .await
        .expect("❌ open_db doit réussir");

    // --- DROP (Soft) ---
    drop_db(&cfg, space, db, DropMode::Soft)
        .await
        .expect("❌ drop_db soft doit réussir");

    assert!(
        !db_root.exists(),
        "❌ Après soft drop, le dossier original ne doit plus exister à son emplacement initial"
    );

    // Vérifie qu'un dossier renommé de sauvegarde existe
    let mut found_soft = false;
    let space_root = cfg.data_root.join(space);
    let mut entries = io::read_dir(&space_root)
        .await
        .expect("❌ Lecture du space_root impossible");

    while let Some(entry) = entries
        .next_entry()
        .await
        .expect("❌ Erreur de lecture d'entrée")
    {
        let p = entry.path();
        let name = p.file_name().unwrap().to_string_lossy();
        if name.starts_with(db) && name.contains(".deleted-") && p.is_dir() {
            found_soft = true;
            break;
        }
    }
    assert!(
        found_soft,
        "❌ Le dossier de sauvegarde *.deleted-<ts> doit exister après un soft drop"
    );

    // --- RE-CREATE & DROP (Hard) ---
    create_db(&cfg, space, db)
        .await
        .expect("❌ recreate_db doit réussir");
    assert!(db_root.exists());

    drop_db(&cfg, space, db, DropMode::Hard)
        .await
        .expect("❌ drop_db hard doit réussir");
    assert!(
        !db_root.exists(),
        "❌ Après hard drop, la DB doit être supprimée définitivement"
    );
}

#[tokio::test]
async fn test_collection_drop_cleans_system_index() {
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let collection = "temp_collection_to_drop";

    // 1. Création de la collection
    mgr.create_collection(collection, None)
        .await
        .expect("❌ Échec de la création de la collection");

    // 2. Vérification physique via la config du stockage isolé
    let col_path = env
        .storage
        .config
        .db_collection_path(&env.space, &env.db, collection);
    assert!(
        col_path.exists(),
        "❌ Le dossier de la collection doit exister"
    );

    // 3. Vérification dans _system.json
    let sys_path = env
        .storage
        .config
        .db_root(&env.space, &env.db)
        .join("_system.json");

    let sys_json: Value = io::read_json(&sys_path)
        .await
        .expect("❌ Lecture _system.json impossible");

    assert!(
        sys_json
            .pointer(&format!("/collections/{}", collection))
            .is_some(),
        "❌ La collection doit être présente dans _system.json"
    );

    // 4. Suppression (Drop)
    mgr.drop_collection(collection)
        .await
        .expect("❌ drop_collection a échoué");

    // 5. Vérification finale
    assert!(
        !col_path.exists(),
        "❌ Le dossier collection doit avoir disparu"
    );

    let sys_json_after: Value = io::read_json(&sys_path)
        .await
        .expect("❌ Lecture _system.json finale impossible");
    assert!(
        sys_json_after
            .pointer(&format!("/collections/{}", collection))
            .is_none(),
        "❌ La collection DOIT être retirée de _system.json après suppression"
    );
}

#[tokio::test]
async fn test_system_index_strict_conformance() {
    // 1. Initialisation de l'environnement
    let env = setup_test_env().await;
    let mgr = CollectionsManager::new(&env.storage, &env.space, &env.db);
    let cfg = &env.storage.config;

    // 2. FORCER LA CRÉATION DU FICHIER SYSTEME
    // On crée une collection bidon pour forcer le manager à persister l'index _system.json
    mgr.create_collection("init_trigger", None)
        .await
        .expect("❌ Échec du trigger de création de l'index système");

    // 3. Localisation du fichier
    let db_root = cfg.db_root(&env.space, &env.db);
    let sys_path = db_root.join("_system.json");

    // Vérification de présence physique
    assert!(
        sys_path.exists(),
        "❌ Le fichier _system.json est toujours absent après création de collection. Dossier : {:?}",
        db_root
    );

    // 4. Lecture et Validation
    let doc: Value = io::read_json(&sys_path)
        .await
        .expect("❌ Lecture ou parsing de _system.json échoué");

    // Métadonnées minimales
    assert!(doc.get("id").is_some(), "❌ L'index système N'A PAS d'ID");

    let expected_schema = format!(
        "db://{}/{}/schemas/v1/db/index.schema.json",
        env.space, env.db
    );
    assert_eq!(
        doc.get("$schema"),
        Some(&json!(expected_schema)),
        "❌ URI de schéma incorrecte"
    );

    // 5. Validation par le registre de schémas
    // Le registre doit être capable de résoudre le schéma (soit localement, soit via fallback système)
    let registry = SchemaRegistry::from_db(cfg, &env.space, &env.db)
        .await
        .expect("❌ Chargement du registre de schémas a échoué");

    let validator = SchemaValidator::compile_with_registry(&expected_schema, &registry)
        .expect("❌ Échec de la compilation du validateur");

    validator
        .validate(&doc)
        .expect("❌ La validation stricte de _system.json a échoué par rapport au schéma");

    println!("✅ 27/27 : Cohérence stricte de l'index système validée.");
}
