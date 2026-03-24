// FICHIER : src-tauri/tests/common/mod.rs

use raise::ai::llm::client::LlmClient;
use raise::json_db::collections::manager::CollectionsManager;
// 🎯 Import massif de notre façade utils::mock
use raise::utils::testing::{inject_collection_schema, inject_mock_component, DbSandbox};

use raise::utils::prelude::*;

static INIT: InitGuard = InitGuard::new();

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

// FICHIER : src-tauri/tests/common/mod.rs

pub async fn setup_test_env(llm_mode: LlmMode) -> UnifiedTestEnv {
    INIT.call_once(|| {
        let _ = tracing_subscriber::fmt().with_test_writer().try_init();
    });

    // 1. ISOLATION : On crée la Sandbox
    let sandbox = DbSandbox::new().await;
    let space = sandbox.config.system_domain.clone();
    let db = sandbox.config.system_db.clone();
    let domain_path = sandbox.config.get_path("PATH_RAISE_DOMAIN").unwrap();

    // 🎯 CRÉATION DU MARQUEUR DE TEST (lu par tools.rs)
    std::fs::write(domain_path.join(".is_test_env"), "1").unwrap();

    // 2. INITIALISATION SIMPLE ET MOCKÉE
    raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

    // 3. PRÉPARATION PHYSIQUE DES SCHÉMAS DE TEST
    inject_custom_test_schemas(&domain_path).await;

    // 4. INITIALISATION DU MANAGER
    let mgr = CollectionsManager::new(&sandbox.storage, &space, &db);
    mgr.init_db()
        .await
        .expect("❌ Échec de l'initialisation de l'index système");

    // 5. INJECTION DES DOCUMENTS DE CONFIGURATION (Data-Driven)

    // A. Config ai_agents
    inject_mock_component(
        &mgr,
        "ai_agents",
        json_value!({
            "target_domain": "un2",
            "system_domain": "_system",
            "system_db": "_system"
        }),
    )
    .await;

    // B. Mapping Ontologique
    let _ = mgr
        .create_collection(
            "configs",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;
    mgr.upsert_document(
        "configs",
        json_value!({
            "_id": "ref:configs:handle:ontological_mapping",
            "handle": "ontological_mapping",
            "mappings": {
                "Class": { "layer": "data", "collection": "classes" },
                "DataType": { "layer": "data", "collection": "types" },
                "Function": { "layer": "sa", "collection": "functions" },
                "SystemFunction": { "layer": "sa", "collection": "functions" },
                "Component": { "layer": "la", "collection": "components" },
                "LogicalComponent": { "layer": "la", "collection": "components" },
                "OperationalActor": { "layer": "oa", "collection": "actors" },
                "OperationalCapability": { "layer": "oa", "collection": "capabilities" },
                "PhysicalNode": { "layer": "pa", "collection": "physical_nodes" },
                "Hardware": { "layer": "pa", "collection": "physical_nodes" },
                "Server": { "layer": "pa", "collection": "physical_nodes" },
                "Requirement": { "layer": "transverse", "collection": "requirements" },
                "TestProcedure": { "layer": "transverse", "collection": "test_procedures" },
                "TestCampaign": { "layer": "transverse", "collection": "test_campaigns" },
                "COTS": { "layer": "epbs", "collection": "configuration_items" },
                "ConfigurationItem": { "layer": "epbs", "collection": "configuration_items" }
            },
            "search_spaces": [
                { "layer": "pa", "collection": "physical_nodes" },
                { "layer": "la", "collection": "components" },
                { "layer": "sa", "collection": "functions" },
                { "layer": "data", "collection": "classes" },
                { "layer": "oa", "collection": "capabilities" },
                { "layer": "oa", "collection": "actors" }
            ]
        }),
    )
    .await
    .unwrap();

    // 6. INITIALISATION LLM
    inject_mock_component(&mgr, "llm", json_value!({ "rust_tokenizer_file": "tokenizer.json", "rust_model_file": "qwen2.5-1.5b-instruct-q4_k_m.gguf" })).await;

    let client = match llm_mode {
        LlmMode::Enabled => {
            let mock_model_file = domain_path.join("_system/ai-assets/models/mock.gguf");
            let _ = fs::ensure_dir_sync(mock_model_file.parent().unwrap());
            let _ = fs::write_sync(&mock_model_file, b"dummy content");
            Some(LlmClient::new(&mgr).await.expect("❌ LlmClient failed"))
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
    fs::create_dir_all_async(&dataset_dir)
        .await
        .expect("Create dataset dir");

    let gps_file = dataset_dir.join("position_gps.json");
    let mock_data = json_value!({ "name": "GPS", "exchangeMechanism": "Flow" });

    fs::write_json_atomic_async(&gps_file, &mock_data)
        .await
        .expect("Write mock data");
    Ok(gps_file)
}
