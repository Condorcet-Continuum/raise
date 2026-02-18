// FICHIER : src-tauri/tests/ai_suite/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager; // Ajout
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Once;

pub mod business_agent_tests;
pub mod data_agent_tests;
pub mod deep_learning_tests;
pub mod epbs_agent_tests;
pub mod hardware_agent_tests;
pub mod llm_tests;
pub mod software_agent_tests;
pub mod system_agent_tests;
pub mod transverse_agent_tests;

static INIT: Once = Once::new();

#[allow(dead_code)]
pub struct AiTestEnv {
    pub storage: StorageEngine,
    pub client: LlmClient,
    pub _space: String,
    pub _db: String,
    pub _tmp_dir: tempfile::TempDir,
}

// CORRECTION E0599 : Passage en async pour pouvoir utiliser .await sur init_db()
pub async fn init_ai_test_env() -> AiTestEnv {
    INIT.call_once(|| {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    });

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let data_root = tmp_dir.path().to_path_buf();

    // On utilise une nouvelle config basée sur le dossier temporaire
    let config = JsonDbConfig {
        data_root: data_root.clone(),
    };

    // IMPORTANT : On utilise "un2" car c'est l'espace par défaut codé en dur
    // dans les prompts des agents pour l'instant.
    let space = "un2".to_string();
    let db = "_system".to_string();

    // 1. Structure de base
    let db_root = config.db_root(&space, &db);
    fs::create_dir_all(&db_root).expect("create db root");

    // 2. COPIE ROBUSTE DES SCHÉMAS (Comme dans code_gen_suite)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let possible_paths = vec![
        manifest_dir.join("../schemas/v1"),
        manifest_dir.join("schemas/v1"),
        PathBuf::from("schemas/v1"),
    ];

    let src_schemas = possible_paths
        .into_iter()
        .find(|p| p.exists())
        .expect("❌ FATAL: Impossible de trouver 'schemas/v1' pour ai_suite.");

    let dest_schemas_root = config.db_schemas_root(&space, &db).join("v1");
    if !dest_schemas_root.exists() {
        fs::create_dir_all(&dest_schemas_root).expect("create schema dir");
    }
    copy_dir_recursive(&src_schemas, &dest_schemas_root).expect("copy schemas");

    // 3. INITIALISATION PROPRE VIA LE MANAGER
    // On crée le moteur
    let storage = StorageEngine::new(config);
    // On instancie le manager pour initialiser la base
    let mgr = CollectionsManager::new(&storage, &space, &db);

    // CORRECTION E0599 : Ajout de .await car init_db() est asynchrone
    mgr.init_db().await.expect("❌ init_db failed in ai_suite");

    // 4. Client LLM
    let gemini_key = env::var("RAISE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("RAISE_MODEL_NAME").ok();
    let local_url =
        env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = LlmClient::new(&local_url, &gemini_key, model_name);

    AiTestEnv {
        storage,
        client,
        _space: space,
        _db: db,
        _tmp_dir: tmp_dir,
    }
}

// Helper indispensable pour la copie
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
