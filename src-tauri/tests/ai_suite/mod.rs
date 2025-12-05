use genaptitude::ai::llm::client::LlmClient;
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};
use std::env;
use std::sync::Once;

static INIT: Once = Once::new();

#[allow(dead_code)]
pub struct AiTestEnv {
    pub storage: StorageEngine,
    pub client: LlmClient,
    // Les champs ont été renommés avec un _ pour éviter les warnings
    pub _space: String,
    pub _db: String,
    pub _tmp_dir: tempfile::TempDir,
}

pub fn init_ai_test_env() -> AiTestEnv {
    INIT.call_once(|| {
        dotenvy::dotenv().ok();
        let _ = tracing_subscriber::fmt()
            .with_env_filter("info")
            .with_test_writer()
            .try_init();
    });

    let tmp_dir = tempfile::tempdir().expect("create temp dir");
    let config = JsonDbConfig::new(tmp_dir.path().to_path_buf());
    let storage = StorageEngine::new(config);

    let gemini_key = env::var("GENAPTITUDE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("GENAPTITUDE_MODEL_NAME").ok();
    let local_url =
        env::var("GENAPTITUDE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = LlmClient::new(&local_url, &gemini_key, model_name);

    AiTestEnv {
        storage,
        client,
        // CORRECTION ICI : Utilisation des noms avec préfixe _
        _space: "test_space".to_string(),
        _db: "_system".to_string(),
        _tmp_dir: tmp_dir,
    }
}
