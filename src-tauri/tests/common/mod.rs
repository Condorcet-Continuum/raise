// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
// 🎯 Import massif de notre façade utils::mock
use raise::utils::mock::{inject_collection_schema, inject_mock_component, DbSandbox};

use raise::utils::{
    io::{self, Path, PathBuf},
    prelude::*,
    Once,
};

static INIT: Once = Once::new();

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum LlmMode {
    Enabled,
    Disabled,
}

#[allow(dead_code)]
pub struct UnifiedTestEnv {
    // 🎯 La Sandbox encapsule StorageEngine, AppConfig et le TempDir !
    pub sandbox: DbSandbox,
    pub client: Option<LlmClient>,
    pub space: String,
    pub db: String,
}

pub async fn setup_test_env(llm_mode: LlmMode) -> UnifiedTestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });

    // 1. ISOLATION : DbSandbox prépare l'environnement de base (index.schema.json, config, db).
    let sandbox = DbSandbox::new().await;

    let space = sandbox.config.system_domain.clone();
    let db = sandbox.config.system_db.clone();
    let domain_path = sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap();

    // 2. INJECTION DES SCHÉMAS DE TEST
    // Au lieu d'écraser l'index système, on utilise la méthode officielle de mock.rs
    // qui va correctement créer les dossiers, les schémas ET les fichiers _meta.json.
    inject_custom_test_schemas(&domain_path).await;

    // 3. INITIALISATION DB & MANAGER
    let mgr = CollectionsManager::new(&sandbox.storage, &space, &db);
    mgr.init_db()
        .await
        .expect("❌ Échec de l'initialisation de l'index système");

    // 4. INJECTION DU MOCK IA EN BASE (Requis pour l'init LLM)
    inject_mock_component(
        &mgr,
        "llm",
        json!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })
    ).await;

    // 5. SATISFAIRE LLMCLIENT AVEC LE MANAGER
    let client = match llm_mode {
        LlmMode::Enabled => {
            let mock_model_file = domain_path.join("_system/ai-assets/models/mock.gguf");
            if let Some(parent) = mock_model_file.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let _ = std::fs::write(&mock_model_file, b"dummy content");

            let isolated_client = LlmClient::new(&mgr)
                .await
                .expect("❌ Impossible d'initialiser le LlmClient isolé");

            Some(isolated_client)
        }
        LlmMode::Disabled => None,
    };

    UnifiedTestEnv {
        sandbox,
        client,
        space,
        db,
    }
}

/// 🎯 DÉLÉGATION À MOCK.RS : Création propre des collections de test
async fn inject_custom_test_schemas(domain_root: &Path) {
    inject_collection_schema(
        domain_root,
        "configuration_items",
        r#"{
        "type": "object",
        "properties": { "name": { "type": "string" }, "exchangeMechanism": { "type": "string" } },
        "additionalProperties": true
    }"#,
    )
    .await;

    inject_collection_schema(
        domain_root,
        "actors",
        r#"{
        "type": "object",
        "properties": {
            "handle": { "type": "string" },
            "displayName": { "type": "string" },
            "kind": { "type": "string" },
            "x_age": { "type": "integer" },
            "x_city": { "type": "string" },
            "x_active": { "type": "boolean" },
            "tags": { "type": "array", "items": { "type": "string" } }
        },
        "additionalProperties": true
    }"#,
    )
    .await;

    inject_collection_schema(
        domain_root,
        "articles",
        r#"{
        "type": "object",
        "properties": {
            "handle": { "type": "string" },
            "title": { "type": "string" }
        },
        "additionalProperties": true
    }"#,
    )
    .await;

    inject_collection_schema(domain_root, "finance", r#"{
        "type": "object",
        "properties": {
            "billing_model": { "type": "string" },
            "revenue_scenarios": { "type": "object" },
            "gross_margin": { "type": "object" },
            "summary": {
                "type": "object",
                "properties": {
                    "net_margin_low": { "type": "number" },
                    "net_margin_mid": { "type": "number" },
                    "mid_is_profitable": { "type": "boolean" },
                    "generated_ref": { "type": "string" }
                }
            }
        },
        "x_rules": [
            {
                "id": "rule-net-low",
                "target": "summary.net_margin_low",
                "expr": { "mul": [ { "var": "revenue_scenarios.low_eur" }, { "var": "gross_margin.low_pct" } ] }
            },
            {
                "id": "rule-net-mid",
                "target": "summary.net_margin_mid",
                "expr": { "mul": [ { "var": "revenue_scenarios.mid_eur" }, { "var": "gross_margin.mid_pct" } ] }
            },
            {
                "id": "rule-profit-check",
                "target": "summary.mid_is_profitable",
                "expr": { "gt": [ { "var": "summary.net_margin_mid" }, { "val": 0 } ] }
            },
            {
                "id": "rule-gen-ref",
                "target": "summary.generated_ref",
                "expr": {
                    "replace": {
                        "value": { "var": "billing_model" },
                        "pattern": { "val": "fixed" },
                        "replacement": { "val": "FIN-2025-OK" }
                    }
                }
            }
        ],
        "additionalProperties": true
    }"#).await;
}

#[allow(dead_code)]
pub async fn seed_mock_datasets(domain_path: &Path) -> RaiseResult<PathBuf> {
    let dataset_dir = domain_path.join("dataset/arcadia/v1/data/exchange-items");
    io::create_dir_all(&dataset_dir)
        .await
        .expect("Create dataset dir");

    let gps_file = dataset_dir.join("position_gps.json");
    let mock_data = json!({ "name": "GPS", "exchangeMechanism": "Flow" });

    io::write_json_atomic(&gps_file, &mock_data)
        .await
        .expect("Write mock data");
    Ok(gps_file)
}
