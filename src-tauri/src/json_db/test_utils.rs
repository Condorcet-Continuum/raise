// FICHIER : src-tauri/src/json_db/test_utils.rs

use crate::json_db::storage::{JsonDbConfig, StorageEngine};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

static INIT: Once = Once::new();

pub const TEST_SPACE: &str = "test_space";
pub const TEST_DB: &str = "test_db";

pub struct TestEnv {
    pub cfg: JsonDbConfig,
    pub storage: StorageEngine,
    pub space: String,
    pub db: String,
    pub tmp_dir: tempfile::TempDir,
}

pub fn init_test_env() -> TestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    });

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let data_root = tmp_dir.path().to_path_buf();

    let cfg = JsonDbConfig {
        data_root: data_root.clone(),
    };

    // 1. Création de la structure de base
    let db_root = cfg.db_root(TEST_SPACE, TEST_DB);
    fs::create_dir_all(&db_root).expect("create db root");

    // 2. COPIE DES SCHÉMAS RÉELS
    // On cherche la racine du repo de manière robuste
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")); // src-tauri

    let possible_paths = vec![
        // Cas 1 : Exécution depuis la racine du workspace
        manifest_dir.join("../schemas/v1"),
        // Cas 2 : Exécution depuis src-tauri
        manifest_dir.join("schemas/v1"),
        // Cas 3 : Fallback relatif
        PathBuf::from("schemas/v1"),
        PathBuf::from("../schemas/v1"),
    ];

    let src_schemas = possible_paths.into_iter().find(|p| p.exists());

    // Destination : <tmp>/test_space/test_db/_system/schemas/v1
    let dest_schemas_root = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");

    if let Some(src) = src_schemas {
        // println!("🧪 TEST: Copie des schémas depuis {:?} vers {:?}", src, dest_schemas_root);

        if !dest_schemas_root.exists() {
            fs::create_dir_all(&dest_schemas_root).expect("create schema dir");
        }

        // Copie récursive du contenu
        copy_dir_recursive(&src, &dest_schemas_root).expect("copy schemas");
    } else {
        eprintln!("⚠️ WARNING: Impossible de trouver le dossier 'schemas/v1'. Les tests sémantiques vont échouer.");
        eprintln!("   Cherché depuis : {:?}", manifest_dir);
    }

    // 3. Initialisation de l'index _system.json (Minimal)
    let system_index_path = cfg.db_root(TEST_SPACE, TEST_DB).join("_system.json");
    if !system_index_path.exists() {
        let minimal_index = serde_json::json!({
            "space": TEST_SPACE,
            "database": TEST_DB,
            "collections": {}
        });
        fs::write(&system_index_path, minimal_index.to_string()).ok();
    }

    // 4. CRÉATION DES DATASETS MOCKS (Correction des chemins)
    let dataset_root = data_root.join("dataset");
    fs::create_dir_all(&dataset_root).unwrap();

    // Mock Article
    let article_rel = "arcadia/v1/data/articles/article.json";
    let article_path = dataset_root.join(article_rel);
    if let Some(p) = article_path.parent() {
        fs::create_dir_all(p).unwrap();
    }

    let mock_article = r#"{
        "handle": "mock-handle",
        "displayName": "Mock Article",
        "slug": "mock-slug",
        "title": "Mock Title",
        "status": "draft",
        "authorId": "00000000-0000-0000-0000-000000000000"
    }"#;
    fs::write(&article_path, mock_article).unwrap();

    // Mock Exchange Item (pour debug_import_exchange_item)
    let ex_item_rel = "arcadia/v1/data/exchange-items/position_gps.json";
    let ex_item_path = dataset_root.join(ex_item_rel);
    if let Some(p) = ex_item_path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(
        &ex_item_path,
        r#"{ "name": "GPS Position", "exchangeMechanism": "Flow" }"#,
    )
    .unwrap();

    // Mock Actor (pour json_db_sql)
    // Note: Le test SQL s'attend à ce que le fichier existe pour le charger,
    // mais seed_actors_from_dataset dans le test s'occupe aussi de l'écriture.
    // On prépare juste le terrain ici.

    let storage = StorageEngine::new(cfg.clone());

    TestEnv {
        cfg,
        storage,
        space: TEST_SPACE.to_string(),
        db: TEST_DB.to_string(),
        tmp_dir,
    }
}

pub fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
    let db_path = cfg.db_root(space, db);
    if !db_path.exists() {
        std::fs::create_dir_all(&db_path).unwrap();
    }
}

pub fn get_dataset_file(cfg: &JsonDbConfig, rel_path: &str) -> PathBuf {
    let root = cfg.data_root.join("dataset");
    let path = root.join(rel_path);

    // CORRECTION : On s'assure que le dossier parent existe.
    // C'est vital pour les tests qui écrivent des fichiers à la volée (ex: json_db_sql).
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).expect("Failed to create dataset parent dir");
        }
    }
    path
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            // On copie tout, pas que les json, au cas où il y a des README ou autres
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
