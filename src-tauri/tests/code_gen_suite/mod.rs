use genaptitude::ai::llm::client::LlmClient;
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};
use std::env;
use std::path::PathBuf;
use std::sync::Once;

static INIT: Once = Once::new();

#[allow(dead_code)]
pub struct CodeGenTestEnv {
    pub storage: StorageEngine,
    pub client: LlmClient,
    pub root_dir: tempfile::TempDir, // Dossier racine (contient db + gen_workspace)
    pub output_path: PathBuf,        // Chemin vers gen_workspace
}

pub fn init_env() -> CodeGenTestEnv {
    INIT.call_once(|| {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    });

    // 1. Création d'un espace de travail temporaire unique
    let root_dir = tempfile::tempdir().expect("create temp dir");

    // 2. Config DB (sous-dossier "db")
    let db_path = root_dir.path().join("genaptitude_db");
    let config = JsonDbConfig::new(db_path);
    let storage = StorageEngine::new(config);

    // 3. Config IA
    let gemini_key = env::var("GENAPTITUDE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("GENAPTITUDE_MODEL_NAME").ok();
    let local_url =
        env::var("GENAPTITUDE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
    let client = LlmClient::new(&local_url, &gemini_key, model_name);

    // 4. Chemin de sortie prévu pour le code
    let output_path = root_dir.path().join("gen_workspace");

    CodeGenTestEnv {
        storage,
        client,
        root_dir,
        output_path,
    }
}
