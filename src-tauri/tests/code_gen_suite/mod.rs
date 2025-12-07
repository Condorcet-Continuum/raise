// FICHIER : src-tauri/tests/code_gen_suite/mod.rs

use genaptitude::ai::llm::client::LlmClient;
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};
use serde_json::json;
use std::env;
use std::fs;
use std::sync::Once;

static INIT: Once = Once::new();

#[allow(dead_code)]
pub struct AiTestEnv {
    pub storage: StorageEngine,
    pub client: LlmClient,
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

    let space = "un2".to_string();
    let db = "_system".to_string();

    // --- BOOTSTRAP DB ---
    let db_root = config.db_root(&space, &db);
    fs::create_dir_all(&db_root).expect("Failed to create db root");

    let schemas_root = config.db_schemas_root(&space, &db).join("v1");

    // CORRECTION : Création de la structure attendue par le SystemAgent (arcadia/oa)
    let oa_dir = schemas_root.join("arcadia").join("oa");
    fs::create_dir_all(&oa_dir).expect("create arcadia/oa schema dir");

    fs::write(
        oa_dir.join("actor.schema.json"),
        json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "handle": { "type": "string" },
                "displayName": { "type": "string" },
                "kind": { "type": "string" },
                "description": { "type": "string" },
                "@context": { "type": "object" },
                "@type": { "type": "string" }
            },
            "additionalProperties": true
        })
        .to_string(),
    )
    .expect("write actor schema");

    // Schema Workunits (resté simple pour l'instant)
    let wu_dir = schemas_root.join("workunits");
    fs::create_dir_all(&wu_dir).expect("create workunits schema dir");
    fs::write(
        wu_dir.join("workunit.schema.json"),
        json!({ "type": "object", "additionalProperties": true }).to_string(),
    )
    .expect("write workunit schema");

    // Index Système (_system.json) mis à jour avec le bon chemin
    let system_index = json!({
        "space": space,
        "database": db,
        "collections": {
            "actors": {
                // CORRECTION : Chemin pointant vers arcadia/oa
                "schema": format!("db://{}/{}/schemas/v1/arcadia/oa/actor.schema.json", space, db),
                "items": []
            },
            "workunits": {
                "schema": format!("db://{}/{}/schemas/v1/workunits/workunit.schema.json", space, db),
                "items": []
            }
        }
    });

    fs::write(
        db_root.join("_system.json"),
        serde_json::to_string_pretty(&system_index).unwrap(),
    )
    .expect("write _system.json");

    let storage = StorageEngine::new(config);

    let gemini_key = env::var("GENAPTITUDE_GEMINI_KEY").unwrap_or_default();
    let model_name = env::var("GENAPTITUDE_MODEL_NAME").ok();
    let local_url =
        env::var("GENAPTITUDE_LOCAL_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());

    let client = LlmClient::new(&local_url, &gemini_key, model_name);

    AiTestEnv {
        storage,
        client,
        _space: space,
        _db: db,
        _tmp_dir: tmp_dir,
    }
}
