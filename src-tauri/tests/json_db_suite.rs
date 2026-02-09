// FICHIER : src-tauri/tests/json_db_suite.rs

use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::utils::{
    async_recursion,
    error::AnyResult,
    fs::{self, Path, PathBuf},
    Once, // Exporté dans mod.rs
};

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

#[path = "json_db_suite/integration_suite.rs"]
pub mod integration_suite;

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

pub async fn init_test_env() -> TestEnv {
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
    fs::ensure_dir(&db_root).await.expect("create db root");

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
    if !fs::exists(&dest_schemas_root).await {
        fs::ensure_dir(&dest_schemas_root).await.unwrap();
    }
    copy_dir_recursive(&src_schemas, &dest_schemas_root)
        .await
        .expect("copy schemas");

    let storage = StorageEngine::new(cfg.clone());
    let mgr = CollectionsManager::new(&storage, TEST_SPACE, TEST_DB);

    // CORRECTION E0599 : init_db() est asynchrone, ajout de .await
    mgr.init_db().await.expect("init_db failed");

    // Mock Datasets
    let dataset_dir = data_root.join("dataset/arcadia/v1/data/exchange-items");
    fs::ensure_dir(&dataset_dir).await.unwrap();
    fs::write_atomic(
        &dataset_dir.join("position_gps.json"),
        br#"{ "name": "GPS", "exchangeMechanism": "Flow" }"#,
    )
    .await
    .unwrap();

    let article_dir = data_root.join("dataset/arcadia/v1/data/articles");
    fs::ensure_dir(&article_dir).await.unwrap();
    fs::write_atomic(
        &article_dir.join("article.json"),
        br#"{ "handle": "test", "displayName": "Test", "status": "draft" }"#,
    )
    .await
    .unwrap();

    TestEnv {
        cfg,
        storage,
        space: TEST_SPACE.to_string(),
        db: TEST_DB.to_string(),
        _tmp_dir: tmp_dir,
    }
}

pub async fn ensure_db_exists(cfg: &JsonDbConfig, space: &str, db: &str) {
    let p = cfg.db_root(space, db);
    if !fs::exists(&p).await {
        fs::ensure_dir(&p).await.unwrap();
    }
}

pub async fn get_dataset_file(cfg: &JsonDbConfig, rel_path: &str) -> PathBuf {
    let path = cfg.data_root.join("dataset").join(rel_path);
    if let Some(p) = path.parent() {
        fs::ensure_dir(p).await.unwrap();
    }
    path
}
#[async_recursion]
async fn copy_dir_recursive(src: &Path, dst: &Path) -> AnyResult<()> {
    if !fs::exists(dst).await {
        fs::ensure_dir(dst).await?;
    }
    let mut entries = fs::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());

        if entry.file_type().await?.is_dir() {
            copy_dir_recursive(&path, &dest_path).await?;
        } else {
            // Utilisation explicite de tokio pour la copie brute de fichier (hors façade)
            tokio::fs::copy(path, dest_path).await?;
        }
    }
    Ok(())
}
