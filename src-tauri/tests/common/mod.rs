// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::utils::config::AppConfig;
use raise::utils::{
    async_recursion,
    io::{self, Path, PathBuf},
    prelude::*,
    Once,
};
use tempfile::TempDir;

static INIT: Once = Once::new();

#[allow(dead_code)]
pub struct UnifiedTestEnv {
    pub storage: StorageEngine,
    pub client: LlmClient,
    pub space: String,
    pub db: String,
    pub domain_path: PathBuf,
    // On conserve le TempDir ici pour garantir que le dossier n'est pas
    // supprimé avant la fin de l'exécution du test.
    pub _tmp_dir: TempDir,
}

pub async fn setup_test_env() -> UnifiedTestEnv {
    // 1. Initialisation unique du logger et de la configuration de base
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();

        std::env::set_var("RAISE_ENV_MODE", "test");

        // --- HACK SSOT : Patch temporaire du JSON pour satisfaire config.rs ---
        let test_config_path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("config.test.json");

        let original_content = std::fs::read_to_string(&test_config_path)
            .expect("Impossible de lire config.test.json");

        let mut json_data: serde_json::Value =
            serde_json::from_str(&original_content).expect("JSON de test invalide");

        if let Some(paths_array) = json_data.get("paths").and_then(|p| p.as_array()) {
            let mut paths_map = serde_json::Map::new();
            for item in paths_array {
                if let (Some(id), Some(val)) = (
                    item.get("id").and_then(|i| i.as_str()),
                    item.get("value").and_then(|v| v.as_str()),
                ) {
                    paths_map.insert(id.to_string(), serde_json::Value::String(val.to_string()));
                }
            }
            json_data["paths"] = serde_json::Value::Object(paths_map);

            std::fs::write(
                &test_config_path,
                serde_json::to_string_pretty(&json_data).unwrap(),
            )
            .expect("Impossible d'écrire le patch temporaire");
        }

        let init_result = AppConfig::init();

        // Restauration immédiate
        std::fs::write(&test_config_path, original_content)
            .expect("Impossible de restaurer config.test.json");

        init_result.expect("Échec de l'initialisation de AppConfig");
    });

    // 2. CRÉATION D'UN CHEMIN UNIQUE PAR THREAD (Isolation Totale)
    // Cela permet de supprimer --test-threads=1
    let test_uuid = uuid::Uuid::new_v4().to_string();
    let temp_dir = tempfile::Builder::new()
        .prefix(&format!("raise_test_{}_", test_uuid))
        .tempdir()
        .expect("❌ Impossible de créer le dossier temporaire pour le test");

    let domain_path = temp_dir.path().to_path_buf();

    let db_config = JsonDbConfig {
        data_root: domain_path.clone(),
    };

    let space = "un2".to_string();
    let db = "_system".to_string();

    let db_root = db_config.db_root(&space, &db);
    io::create_dir_all(&db_root).await.expect("create db root");

    // 3. Copie des schémas JSON DB
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let possible_paths = vec![
        manifest_dir.join("../schemas/v1"),
        manifest_dir.join("schemas/v1"),
        PathBuf::from("schemas/v1"),
    ];

    let src_schemas = possible_paths
        .into_iter()
        .find(|p| p.exists())
        .expect("❌ FATAL: Impossible de trouver 'schemas/v1'.");

    let dest_schemas_root = db_config.db_schemas_root(&space, &db).join("v1");
    if !dest_schemas_root.exists() {
        io::create_dir_all(&dest_schemas_root)
            .await
            .expect("create schema dir");
    }
    copy_dir_recursive(&src_schemas, &dest_schemas_root)
        .await
        .expect("copy schemas");

    // 4. Initialisation du Storage et du Manager
    let storage = StorageEngine::new(db_config);
    let mgr = CollectionsManager::new(&storage, &space, &db);
    mgr.init_db()
        .await
        .expect("❌ init_db failed in test environment");

    let app_config = AppConfig::get();

    let gemini_key = app_config
        .ai_engines
        .get("cloud_gemini")
        .and_then(|e| e.api_key.clone())
        .unwrap_or_default();

    let model_name = app_config
        .ai_engines
        .get("primary_local")
        .map(|e| e.model_name.clone());

    let local_url = app_config
        .ai_engines
        .get("primary_local")
        .and_then(|e| e.api_url.clone())
        .unwrap_or_else(|| "http://localhost:8081".to_string());

    let client = LlmClient::new(&local_url, &gemini_key, model_name);

    UnifiedTestEnv {
        storage,
        client,
        space,
        db,
        domain_path,
        _tmp_dir: temp_dir,
    }
}

#[async_recursion]
async fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    if !dst.exists() {
        io::create_dir_all(dst).await?;
    }

    let mut entries = io::read_dir(src).await?;
    while let Some(entry) = entries.next_entry().await? {
        let ty = entry.file_type().await?;
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst.join(entry.file_name())).await?;
        } else {
            io::copy(entry.path(), dst.join(entry.file_name())).await?;
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub async fn seed_mock_datasets(domain_path: &Path) -> Result<PathBuf> {
    let dataset_dir = domain_path.join("dataset/arcadia/v1/data/exchange-items");
    io::create_dir_all(&dataset_dir)
        .await
        .expect("Impossible de créer le dossier dataset");

    let gps_file = dataset_dir.join("position_gps.json");

    let mock_data = json!({
        "name": "GPS",
        "exchangeMechanism": "Flow"
    });

    io::write_json_atomic(&gps_file, &mock_data)
        .await
        .expect("Impossible d'écrire le mock");

    Ok(gps_file)
}
