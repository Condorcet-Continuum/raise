// FICHIER : src-tauri/tests/json_db_suite.rs

use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

// --- DÉCLARATION EXPLICITE DES MODULES ---
// On dit à Rust exactement où trouver chaque fichier dans le sous-dossier

#[path = "json_db_suite/dataset_integration.rs"]
pub mod dataset_integration;

#[path = "json_db_suite/json_db_errors.rs"]
pub mod json_db_errors;

#[path = "json_db_suite/json_db_idempotent.rs"]
pub mod json_db_idempotent;

#[path = "json_db_suite/json_db_integration.rs"]
pub mod json_db_integration;

#[path = "json_db_suite/json_db_lifecycle.rs"]
pub mod json_db_lifecycle;

#[path = "json_db_suite/json_db_query_integration.rs"]
pub mod json_db_query_integration;

#[path = "json_db_suite/json_db_sql.rs"]
pub mod json_db_sql;

#[path = "json_db_suite/json_db_indexes_ops.rs"]
pub mod json_db_indexes_ops;

#[path = "json_db_suite/schema_consistency.rs"]
pub mod schema_consistency;

#[path = "json_db_suite/schema_minimal.rs"]
pub mod schema_minimal;

#[path = "json_db_suite/workunits_x_compute.rs"]
pub mod workunits_x_compute;

// --- ENVIRONNEMENT DE TEST (Commun à tous) ---

static INIT: Once = Once::new();

pub const TEST_SPACE: &str = "un2";
pub const TEST_DB: &str = "_system";

pub struct TestEnv {
    pub cfg: JsonDbConfig,
    pub storage: StorageEngine,
    pub space: String,
    pub db: String,
    pub _tmp_dir: tempfile::TempDir,
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

    let db_root = cfg.db_root(TEST_SPACE, TEST_DB);
    fs::create_dir_all(&db_root).expect("create db root");

    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let possible_paths = vec![
        manifest_dir.join("../schemas/v1"),
        manifest_dir.join("schemas/v1"),
        PathBuf::from("schemas/v1"),
    ];
    let src_schemas = possible_paths
        .into_iter()
        .find(|p| p.exists())
        .expect("❌ FATAL: Impossible de trouver 'schemas/v1'");

    let dest_schemas_root = cfg.db_schemas_root(TEST_SPACE, TEST_DB).join("v1");
    if !dest_schemas_root.exists() {
        fs::create_dir_all(&dest_schemas_root).unwrap();
    }
    copy_dir_recursive(&src_schemas, &dest_schemas_root).expect("copy schemas");

    let storage = StorageEngine::new(cfg.clone());
    let mgr = CollectionsManager::new(&storage, TEST_SPACE, TEST_DB);
    mgr.init_db().expect("init_db failed");

    // Mock Datasets
    let dataset_dir = data_root.join("dataset/arcadia/v1/data/exchange-items");
    fs::create_dir_all(&dataset_dir).unwrap();
    fs::write(
        dataset_dir.join("position_gps.json"),
        r#"{ "name": "GPS", "exchangeMechanism": "Flow" }"#,
    )
    .unwrap();

    let article_dir = data_root.join("dataset/arcadia/v1/data/articles");
    fs::create_dir_all(&article_dir).unwrap();
    fs::write(
        article_dir.join("article.json"),
        r#"{ "handle": "test", "displayName": "Test", "status": "draft" }"#,
    )
    .unwrap();

    TestEnv {
        cfg,
        storage,
        space: TEST_SPACE.to_string(),
        db: TEST_DB.to_string(),
        _tmp_dir: tmp_dir,
    }
}

pub fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
    let p = cfg.db_root(space, db);
    if !p.exists() {
        fs::create_dir_all(p).unwrap();
    }
}

pub fn get_dataset_file(cfg: &JsonDbConfig, rel_path: &str) -> PathBuf {
    let path = cfg.data_root.join("dataset").join(rel_path);
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    path
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    if !dst.exists() {
        fs::create_dir_all(dst)?;
    }
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.join(entry.file_name()))?;
        }
    }
    Ok(())
}
