// FICHIER : src-tauri/src/utils/testing/mock.rs
#![cfg(any(test, debug_assertions))]

// 1. Core : Concurrence, Mémoire et Identifiants
use crate::raise_error;
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{RuntimeEnv, SharedRef, UniqueId, UtcClock};
use crate::utils::io::fs::{self, tempdir, Path, PathBuf, TempDir};

// 2. Data : Configuration, JSON et Traits
// 🎯 FIX : On importe BOOTSTRAP_DB et BOOTSTRAP_DOMAIN au lieu de SYSTEM_...
use crate::utils::data::config::{
    AppConfig, CoreConfig, DbPointer, DeepLearningConfig, IntegrationsConfig, MountPointsConfig,
    SimulationContextConfig, WorldModelConfig, BOOTSTRAP_DB, BOOTSTRAP_DOMAIN, CONFIG,
};
use crate::utils::data::json::{self, json_value, JsonValue};
use crate::utils::data::UnorderedMap;

// 4. Dépendances métier (Base de données JSON)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};

pub const MOCK_LLM_MODEL: &str = "Qwen2.5-7B-Instruct-Q4_K_M.gguf";
pub const MOCK_LLM_TOKENIZER: &str = "tokenizer.json";

// --- DÉFINITION DES SCHÉMAS STANDARDS POUR TESTS ---

// Dans mock.rs

pub const SESSION_SCHEMA_MOCK: &str = r#"{
    "type": "object",
    "properties": {
        "_id": { 
            "type": "string",
            "x_compute": {
                "engine": "plan/v1",
                "scope": "root",
                "update": "if_missing",
                "plan": { "op": "uuid_v4" }
            }
        },
        "_created_at": { 
            "type": "string",
            "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "if_missing" }
        },
        "_updated_at": { 
            "type": "string",
            "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "always" }
        },
        "@type": { "type": "array", "items": { "type": "string" } },
        "user_id": { "type": "string" },
        "user_handle": { "type": "string" },
        "status": { "type": "string", "enum": ["active", "idle", "expired", "revoked"] },
        "expires_at": { "type": "string", "format": "date-time" },
        "last_activity_at": { "type": "string", "format": "date-time" },
        "context": { 
            "type": "object",
            "required": ["current_domain", "current_db", "active_dapp_id"] 
        }
    },
    "required": ["user_id", "status", "context"]
}"#;

pub const ACTORS_SCHEMA_MOCK: &str =
    r#"{ "type": "object", "properties": { "handle": { "type": "string" } } }"#;
pub const ARTICLES_SCHEMA_MOCK: &str =
    r#"{ "type": "object", "properties": { "title": { "type": "string" } } }"#;
pub const CONFIG_ITEMS_SCHEMA_MOCK: &str =
    r#"{ "type": "object", "properties": { "name": { "type": "string" } } }"#;
pub const FINANCE_SCHEMA_MOCK: &str = r#"{
    "type": "object",
    "x_rules": [
        { 
            "_id": "rule_net_margin_low",
            "target": "summary.net_margin_low", 
            "expr": { "mul": [ { "var": "revenue_scenarios.low_eur" }, { "var": "gross_margin.low_pct" } ] }
        },
        { 
            "_id": "rule_net_margin_mid",
            "target": "summary.net_margin_mid", 
            "expr": { "mul": [ { "var": "revenue_scenarios.mid_eur" }, { "var": "gross_margin.mid_pct" } ] }
        },
        { 
            "_id": "rule_mid_profitable",
            "target": "summary.mid_is_profitable", 
            "expr": { "gt": [ { "var": "summary.net_margin_mid" }, { "val": 0 } ] }
        },
        { 
            "_id": "rule_gen_ref",
            "target": "summary.generated_ref", 
            "expr": {
                "replace": {
                    "value": { "var": "billing_model" },
                    "pattern": { "val": "fixed" },
                    "replacement": { "val": "FIN-2025-OK" }
                }
            }
        }
    ]
}"#;

pub const USER_SCHEMA_MOCK: &str = r#"{
    "type": "object",
    "properties": {
        "_id": {
            "type": "string",
            "x_compute": {
                "engine": "plan/v1",
                "scope": "root",
                "update": "if_missing",
                "plan": { "op": "uuid_v4" }
            }
        },
        "handle": { "type": "string" },
        "name": { "type": "object" },
        "default_domain": { "type": "string" },
        "default_db": { "type": "string" },
        "role": { "type": "string" }
    },
    "required": ["_id", "handle",  "name", "default_domain", "default_db"]
}"#;

// =========================================================================
// 🔧 UTILS DE CONFIGURATION DE TEST
// =========================================================================

pub fn create_default_test_config() -> AppConfig {
    let mut paths = UnorderedMap::new();
    let tmp = RuntimeEnv::temp_dir();
    paths.insert(
        "PATH_RAISE_DOMAIN".to_string(),
        tmp.to_string_lossy().to_string(),
    );
    paths.insert(
        "PATH_LOGS".to_string(),
        tmp.join("logs").to_string_lossy().to_string(),
    );

    AppConfig {
        id: UniqueId::new_v4().to_string(),
        created_at: UtcClock::now().to_rfc3339(),
        updated_at: UtcClock::now().to_rfc3339(),
        semantic_type: vec!["SystemConfig".to_string(), "cfg:SystemConfig".to_string()],
        name: Some(UnorderedMap::from([(
            "en".to_string(),
            "Default Test Config".to_string(),
        )])),
        // 🎯 FIX : Utilisation stricte de BOOTSTRAP_DOMAIN et BOOTSTRAP_DB
        mount_points: MountPointsConfig {
            system: DbPointer {
                domain: BOOTSTRAP_DOMAIN.to_string(),
                db: BOOTSTRAP_DB.to_string(),
            },
            raise: DbPointer {
                domain: BOOTSTRAP_DOMAIN.to_string(),
                db: "raise_core".to_string(),
            },
            exploration: DbPointer {
                domain: "project_x".into(),
                db: "sandbox".into(),
            },
            modeling: DbPointer {
                domain: "project_x".into(),
                db: "mbse".into(),
            },
            simulation: DbPointer {
                domain: "project_x".into(),
                db: "sim_mbse".into(),
            },
            integration: DbPointer {
                domain: "project_x".into(),
                db: "test_mbse".into(),
            },
            production: DbPointer {
                domain: "project_x".into(),
                db: "prod_mbse".into(),
            },
            operation: DbPointer {
                domain: "project_x".into(),
                db: "telemetry".into(),
            },
        },
        workstation: None,
        user: None,
        core: CoreConfig {
            env_mode: "test".to_string(),
            graph_mode: "none".to_string(),
            log_level: "debug".to_string(),
            vector_store_provider: "memory".to_string(),
            language: "en".to_string(),
        },
        world_model: WorldModelConfig::default(),
        deep_learning: DeepLearningConfig::default(),
        paths,
        active_dapp_id: "mock-dapp-id".to_string(),
        workstation_id: "mock-workstation-id".to_string(),
        active_services: vec!["mock-service-id".to_string()],
        active_components: vec!["mock-comp-id".to_string()],
        integrations: IntegrationsConfig::default(),
        simulation_context: SimulationContextConfig::default(),
    }
}

pub fn load_test_sandbox() -> RaiseResult<AppConfig> {
    let manifest = match RuntimeEnv::var("CARGO_MANIFEST_DIR") {
        Ok(v) => v,
        Err(e) => raise_error!(
            "ERR_CONFIG_ENV_MANIFEST",
            error = e,
            context = json_value!({ "var": "CARGO_MANIFEST_DIR" })
        ),
    };

    let path = PathBuf::from(manifest).join("tests/config.test.json");

    if !path.exists() {
        return Ok(create_default_test_config());
    }

    let content = match fs::read_to_string_sync(&path) {
        Ok(c) => c,
        Err(e) => raise_error!(
            "ERR_CONFIG_FS_READ",
            error = e,
            context = json_value!({ "path": path.to_string_lossy() })
        ),
    };

    let mut config: AppConfig = match json::deserialize_from_str(&content) {
        Ok(cfg) => cfg,
        Err(e) => raise_error!(
            "ERR_CONFIG_PARSE",
            error = e,
            context = json_value!({ "path": path.to_string_lossy() })
        ),
    };

    if let Some(domain_path) = config.paths.get_mut("PATH_RAISE_DOMAIN") {
        let temp_dir = RuntimeEnv::temp_dir();
        let temp_str = temp_dir.to_string_lossy();

        if domain_path.starts_with("/tmp") || domain_path.contains(temp_str.as_ref() as &str) {
            let unique_id = format!(
                "{}_{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_micros()
            );
            *domain_path = format!("{}_{}", domain_path, unique_id);
            let _ = fs::create_dir_all_sync(domain_path);
        }
    }

    // 🎯 FIX : Sécurisation du Mount Point système de la Sandbox avec BOOTSTRAP_*
    config.mount_points.system.domain = BOOTSTRAP_DOMAIN.to_string();
    config.mount_points.system.db = BOOTSTRAP_DB.to_string();

    Ok(config)
}

// --- FONCTIONS MOCKS ---

pub async fn inject_core_schemas_to_index(db_cfg: &JsonDbConfig, sys_doc: &mut JsonValue) {
    let base_uri = format!("db://{}/{}", BOOTSTRAP_DOMAIN, BOOTSTRAP_DB);
    let schemas_dir = db_cfg
        .db_schemas_root(BOOTSTRAP_DOMAIN, BOOTSTRAP_DB)
        .join("v1/db");
    let _ = fs::create_dir_all_async(&schemas_dir).await;

    if sys_doc.get("schemas").is_none() {
        *sys_doc = json_value!({ "schemas": { "v1": {}, "v2": {} } });
    }
    let schemas_v1 = sys_doc["schemas"]["v1"].as_object_mut().unwrap();

    // 1. Migration
    let migration_schema = json_value!({
        "$id": format!("{}/schemas/v2/system/db/migration.schema.json", base_uri),
        "type": "object",
        "properties": {
            "$schema": { "type": "string" },
            "_id": { "type": "string", "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" } },
            "handle": { "type": "string" },
            "name": { "type": "object" },
            "version": { "type": "string" },
            "description": { "type": "string" },
            "applied_at": { "type": "string" }
        },
        "required": ["$schema", "_id", "handle", "name", "version", "description", "applied_at"]
    });
    let _ = fs::write_json_atomic_async(
        &schemas_dir.join("migration.schema.json"),
        &migration_schema,
    )
    .await;
    schemas_v1.insert(
        "db/migration.schema.json".to_string(),
        json_value!({ "file": "v1/db/migration.schema.json" }),
    );

    // 2. Index (Core)
    let core_schema = json_value!({
        "$id": format!("{}/schemas/v1/db/index.schema.json", base_uri),
        "type": "object",
        "properties": {
            "_id": { "type": "string", "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" } },
            "name": { "type": "string" },
            "space": { "type": "string" },
            "database": { "type": "string" },
            "version": { "type": "integer", "default": 1 },
            "collections": {
                "type": "object",
                "properties": {
                    "_migrations": {
                        "type": "object",
                        "default": { "schema": format!("{}/schemas/v1/db/migration.schema.json", base_uri), "items": [] }
                    }
                },
                "default": {}
            },
            "rules": { "type": "object", "default": {} },
            "schemas": { "type": "object", "default": { "v1": {} } }
        },
        "required": ["_id", "name", "space", "database"]
    });
    let _ = fs::write_json_atomic_async(&schemas_dir.join("index.schema.json"), &core_schema).await;
    schemas_v1.insert(
        "db/index.schema.json".to_string(),
        json_value!({ "file": "v1/db/index.schema.json" }),
    );

    // 3. Generic
    let generic_schema = json_value!({
        "$id": format!("{}/schemas/v1/db/generic.schema.json", base_uri),
        "type": "object",
        "properties": {
            "_id": { "type": "string", "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" } },
            "_created_at": { "type": "string", "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "if_missing" } },
            "_updated_at": { "type": "string", "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "always" } }
        },
        "required": ["_id"],
        "additionalProperties": true
    });
    let _ = fs::write_json_atomic_async(&schemas_dir.join("generic.schema.json"), &generic_schema)
        .await;
    schemas_v1.insert(
        "db/generic.schema.json".to_string(),
        json_value!({ "file": "v1/db/generic.schema.json" }),
    );
}

pub async fn inject_mock_schema_to_index(
    db_cfg: &JsonDbConfig,
    sys_doc: &mut JsonValue,
    collection_name: &str,
    content: &str,
) {
    if sys_doc.get("schemas").is_none() {
        *sys_doc = json_value!({ "schemas": { "v1": {}, "v2": {} } });
    }

    let schemas_dir = db_cfg
        .db_schemas_root(BOOTSTRAP_DOMAIN, BOOTSTRAP_DB)
        .join("v1/mock");
    let _ = fs::create_dir_all_async(&schemas_dir).await;

    let schemas_v1 = sys_doc["schemas"]["v1"].as_object_mut().unwrap();
    let mut json_val: JsonValue = json::deserialize_from_str(content).unwrap_or(json_value!({}));

    let schema_uri = format!(
        "db://{}/{}/schemas/v1/mock/{}.schema.json",
        BOOTSTRAP_DOMAIN, BOOTSTRAP_DB, collection_name
    );

    if let Some(obj) = json_val.as_object_mut() {
        obj.insert("$id".to_string(), JsonValue::String(schema_uri.clone()));
    }

    let _ = fs::write_json_atomic_async(
        &schemas_dir.join(format!("{}.schema.json", collection_name)),
        &json_val,
    )
    .await;
    schemas_v1.insert(
        format!("mock/{}.schema.json", collection_name),
        json_value!({ "file": format!("v1/mock/{}.schema.json", collection_name) }),
    );
}

pub async fn inject_v2_schema_mock(
    db_cfg: &JsonDbConfig,
    sys_doc: &mut JsonValue,
    logical_path: &str, // ex: "assurance/quality_report"
) {
    if sys_doc.get("schemas").is_none() {
        *sys_doc = json_value!({ "schemas": { "v1": {}, "v2": {} } });
    }
    let schemas_v2 = sys_doc["schemas"]["v2"].as_object_mut().unwrap();

    // 1. Détermination des chemins virtuels
    let file_name = format!(
        "{}.schema.json",
        Path::new(logical_path)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
    );
    let parent_dir = Path::new(logical_path).parent().unwrap_or(Path::new(""));
    let schemas_dir = db_cfg
        .db_schemas_root(BOOTSTRAP_DOMAIN, BOOTSTRAP_DB)
        .join("v2")
        .join(parent_dir);

    let _ = fs::create_dir_all_async(&schemas_dir).await;

    // 2. Création d'un schéma fantôme (Valide mais vide)
    let schema_uri = format!(
        "db://{}/{}/schemas/v2/{}.schema.json",
        BOOTSTRAP_DOMAIN, BOOTSTRAP_DB, logical_path
    );

    let schema_mock = json_value!({
        "$id": schema_uri,
        "type": "object",
        "properties": {
            "_id": { "type": "string", "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" } },
            "_created_at": { "type": "string", "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "if_missing" } },
            "_updated_at": { "type": "string", "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "always" } }
        },
        "required": ["_id"],
        "additionalProperties": true
    });

    // 3. Écriture sur le disque de la Sandbox
    let _ = fs::write_json_atomic_async(&schemas_dir.join(&file_name), &schema_mock).await;

    // 4. Inscription officielle dans l'index de test
    schemas_v2.insert(
        format!("{}.schema.json", logical_path),
        json_value!({ "file": format!("v2/{}.schema.json", logical_path) }),
    );
}

pub async fn inject_mock_user(manager: &CollectionsManager<'_>, userhandle: &str) {
    let user_doc = json_value!({
        "handle": userhandle,
        "name": { "fr": userhandle, "en": userhandle },
        "default_domain": "mbse2",
        "default_db": "drones",
        "role": "engineer"
    });

    match manager.insert_with_schema("users", user_doc).await {
        Ok(_) => {}
        Err(e) => panic!("Échec de l'injection de l'agent de test : {:?}", e),
    }
}

pub async fn inject_mock_component(
    manager: &CollectionsManager<'_>,
    comp_id: &str,
    mut settings: JsonValue,
) {
    let real_handle = match comp_id {
        "llm" => "ai_llm",
        "voice" => "ai_voice",
        "nlp" => "ai_nlp",
        other => other,
    };

    if real_handle == "ai_llm" {
        let models_dir = dirs::home_dir()
            .unwrap_or_default()
            .join("raise_domain/_system/ai-assets/models");

        if settings["rust_model_file"].is_null() {
            settings["rust_model_file"] = json_value!(models_dir
                .join(MOCK_LLM_MODEL)
                .to_string_lossy()
                .to_string());
        }
        if settings["rust_tokenizer_file"].is_null() {
            settings["rust_tokenizer_file"] = json_value!(models_dir
                .join(MOCK_LLM_TOKENIZER)
                .to_string_lossy()
                .to_string());
        }
    }

    let ref_id = format!("ref:components:handle:{}", real_handle);

    let _ = manager
        .create_collection("service_configs", "/v1/db/generic.schema.json")
        .await;

    let doc = json_value!({
        "_id": format!("mock_config_{}", real_handle),
        "handle": format!("mock_config_{}", real_handle),
        "service_id": "ref:services:handle:ai",
        "environment": "test",
        "component_settings": {
            ref_id: settings
        }
    });

    match manager.insert_raw("service_configs", &doc).await {
        Ok(_) => {}
        Err(e) => panic!(
            "❌ Échec critique lors de l'injection de la configuration Mock pour {} : {:?}",
            real_handle, e
        ),
    }
}

pub async fn inject_schema_to_path(db_cfg: &JsonDbConfig) {
    // 🎯 Dynamique selon la config
    let schema_dir = db_cfg
        .db_schemas_root(BOOTSTRAP_DOMAIN, BOOTSTRAP_DB)
        .join("v1/db");
    let _ = fs::create_dir_all_async(&schema_dir).await;

    let base_uri = format!("db://{}/{}", BOOTSTRAP_DOMAIN, BOOTSTRAP_DB);

    let migration_schema = json_value!({
        "$id": format!("{}/schemas/v2/system/db/migration.schema.json", base_uri),
        "type": "object",
        "properties": {
            "$schema": { "type": "string" },
            "_id": {
                "type": "string",
                "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" }
            },
            "handle": { "type": "string" },
            "name": { "type": "object" },
            "version": { "type": "string" },
            "description": { "type": "string" },
            "applied_at": { "type": "string" }
        },
        "required": ["$schema", "_id", "handle", "name", "version", "description", "applied_at"]
    });
    let _ =
        fs::write_json_atomic_async(&schema_dir.join("migration.schema.json"), &migration_schema)
            .await;

    let core_schema = json_value!({
        "$id": format!("{}/schemas/v1/db/index.schema.json", base_uri),
        "type": "object",
        "properties": {
            "_id": {
                "type": "string",
                "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" }
            },
            "name": { "type": "string" },
            "space": { "type": "string" },
            "database": { "type": "string" },
            "version": { "type": "integer", "default": 1 },
            "collections": {
                "type": "object",
                "properties": {
                    "_migrations": {
                        "type": "object",
                        "default": {
                            "schema": format!("{}/schemas/v1/db/migration.schema.json", base_uri),
                            "items": []
                        }
                    }
                },
                "default": {}
            },
            "rules": {
                "type": "object",
                "properties": {
                    "_system_rules": {
                        "type": "object",
                        "default": {
                            "schema": format!("{}/schemas/v1/db/rule.schema.json", base_uri),
                            "items": []
                        }
                    }
                },
                "default": {}
            },
            "schemas": { "type": "object", "default": { "v1": {} } }
        },
        "required": ["_id", "name", "space", "database"]
    });
    let _ = fs::write_json_atomic_async(&schema_dir.join("index.schema.json"), &core_schema).await;

    let generic_schema = json_value!({
        "$id": format!("{}/schemas/v1/db/generic.schema.json", base_uri),
        "type": "object",
        "properties": {
            "_id": {
                "type": "string",
                "x_compute": { "plan": { "op": "uuid_v4" }, "update": "if_missing" }
            },
            "_created_at": {
                "type": "string",
                "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "if_missing" }
            },
            "_updated_at": {
                "type": "string",
                "x_compute": { "plan": { "op": "now_rfc3339" }, "update": "always" }
            },
            "_p2p": {
                "type": "object",
                "properties": {
                    "revision": { "type": "integer", "default": 1 },
                    "origin_node": { "type": "string" },
                    "checksum": { "type": "string" }
                },
                "default": { "revision": 1 }
            }
        },
        "required": ["_id"],
        "additionalProperties": true
    });
    let _ =
        fs::write_json_atomic_async(&schema_dir.join("generic.schema.json"), &generic_schema).await;
}

pub async fn inject_collection_schema(domain_root: &Path, collection_name: &str, content: &str) {
    let schemas_dir = domain_root.join(format!(
        "{}/{}/schemas/v1/mock",
        BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
    ));
    let _ = fs::create_dir_all_async(&schemas_dir).await;

    let schema_uri = format!(
        "db://{}/{}/schemas/v1/mock/{}.schema.json",
        BOOTSTRAP_DOMAIN, BOOTSTRAP_DB, collection_name
    );
    let schema_file = schemas_dir.join(format!("{}.schema.json", collection_name));

    let mut json_val: JsonValue = match json::deserialize_from_str(content) {
        Ok(v) => v,
        Err(_) => json_value!({}),
    };

    if let Some(obj) = json_val.as_object_mut() {
        obj.insert("$id".to_string(), JsonValue::String(schema_uri.clone()));
    }

    let _ = fs::write_async(&schema_file, json_val.to_string().as_bytes()).await;

    let col_dir = domain_root
        .join(format!("{}/{}/collections", BOOTSTRAP_DOMAIN, BOOTSTRAP_DB))
        .join(collection_name);
    let _ = fs::create_dir_all_async(&col_dir).await;

    let meta_content = json_value!({
        "schema": schema_uri,
        "indexes": []
    });

    let _ = fs::write_async(
        &col_dir.join("_meta.json"),
        meta_content.to_string().as_bytes(),
    )
    .await;
}

pub async fn inject_mock_config() {
    if CONFIG.get().is_none() {
        let config = create_default_test_config();
        let _ = CONFIG.set(config);
    }
    if crate::utils::data::config::DEVICE.get().is_none() {
        let test_device = if cfg!(feature = "cuda") {
            candle_core::Device::new_cuda(0).unwrap_or(candle_core::Device::Cpu)
        } else {
            candle_core::Device::Cpu
        };

        let _ = crate::utils::data::config::DEVICE.set(test_device);
        println!(
            "🧪 [Raise Test] Device injecté : {:?}",
            crate::utils::data::config::DEVICE.get()
        );
    }
}

// --- SANDBOXES ---
pub struct DbSandbox {
    _dir: TempDir,
    pub storage: StorageEngine,
    pub config: AppConfig,
}

impl DbSandbox {
    pub async fn new() -> Self {
        inject_mock_config().await;
        let mut config = AppConfig::get().clone();

        let dir = match tempdir() {
            Ok(d) => d,
            Err(e) => panic!("Création du dossier temporaire échouée : {:?}", e),
        };
        let root_path = dir.path().to_path_buf();

        config.paths.insert(
            "PATH_RAISE_DOMAIN".to_string(),
            root_path.to_string_lossy().to_string(),
        );

        let db_cfg = JsonDbConfig::new(root_path.clone());

        // 🎯 NOUVELLE LOGIQUE DDL : On prépare l'index de test
        let sys_path = db_cfg
            .db_root(BOOTSTRAP_DOMAIN, BOOTSTRAP_DB)
            .join("_system.json");
        fs::ensure_dir_async(sys_path.parent().unwrap())
            .await
            .unwrap();

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/index.schema.json",
            BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
        );
        let mut initial_system_doc = json_value!({
            "$schema": schema_uri,
            "name": format!("{}_{}", BOOTSTRAP_DOMAIN, BOOTSTRAP_DB),
            "space": BOOTSTRAP_DOMAIN,
            "database": BOOTSTRAP_DB,
            "schemas": { "v1": {}, "v2": {} }
        });

        inject_core_schemas_to_index(&db_cfg, &mut initial_system_doc).await;
        inject_mock_schema_to_index(
            &db_cfg,
            &mut initial_system_doc,
            "sessions",
            SESSION_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(&db_cfg, &mut initial_system_doc, "users", USER_SCHEMA_MOCK)
            .await;
        inject_mock_schema_to_index(
            &db_cfg,
            &mut initial_system_doc,
            "actors",
            ACTORS_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(
            &db_cfg,
            &mut initial_system_doc,
            "articles",
            ARTICLES_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(
            &db_cfg,
            &mut initial_system_doc,
            "configuration_items",
            CONFIG_ITEMS_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(
            &db_cfg,
            &mut initial_system_doc,
            "finance",
            FINANCE_SCHEMA_MOCK,
        )
        .await;

        inject_v2_schema_mock(&db_cfg, &mut initial_system_doc, "assurance/quality_report").await;
        inject_v2_schema_mock(&db_cfg, &mut initial_system_doc, "assurance/xai_frame").await;
        inject_v2_schema_mock(&db_cfg, &mut initial_system_doc, "common/types/base").await;

        inject_v2_schema_mock(
            &db_cfg,
            &mut initial_system_doc,
            "agents/memory/vector_store_record",
        )
        .await;
        inject_v2_schema_mock(
            &db_cfg,
            &mut initial_system_doc,
            "agents/memory/chat_session",
        )
        .await;
        // On écrit le fichier avec les schémas AVANT de lancer le CollectionsManager
        fs::write_json_atomic_async(&sys_path, &initial_system_doc)
            .await
            .unwrap();

        let storage = StorageEngine::new(db_cfg);
        let sandbox = Self {
            _dir: dir,
            storage,
            config,
        };

        let mgr = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        let base_uri = format!("db://{}/{}", BOOTSTRAP_DOMAIN, BOOTSTRAP_DB);

        // Ceci fonctionnera car la DB existe et contient les schémas DDL
        let _ = mgr
            .init_db_with_schema(&format!("{}/schemas/v1/db/index.schema.json", base_uri))
            .await;

        let _ = mgr
            .create_collection(
                "users",
                &format!("{}/schemas/v1/mock/users.schema.json", base_uri),
            )
            .await;
        let _ = mgr
            .create_collection(
                "sessions",
                &format!("{}/schemas/v1/mock/sessions.schema.json", base_uri),
            )
            .await;

        sandbox
    }

    pub async fn mock_db(manager: &CollectionsManager<'_>) -> RaiseResult<bool> {
        // Pré-injection vitale pour les tests ciblés sur d'autres espaces (space_test, db_test)
        let sys_path = manager
            .storage
            .config
            .db_root(&manager.space, &manager.db)
            .join("_system.json");
        if !fs::exists_async(&sys_path).await {
            fs::ensure_dir_async(sys_path.parent().unwrap())
                .await
                .unwrap();
            let schema_uri = format!(
                "db://{}/{}/schemas/v1/db/index.schema.json",
                BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
            );
            let mut initial_system_doc = json_value!({
                "$schema": schema_uri,
                "name": format!("{}_{}", manager.space, manager.db),
                "space": manager.space,
                "database": manager.db,
                "schemas": { "v1": {}, "v2": {} }
            });
            let db_cfg = &manager.storage.config;
            inject_core_schemas_to_index(db_cfg, &mut initial_system_doc).await;
            inject_mock_schema_to_index(db_cfg, &mut initial_system_doc, "users", USER_SCHEMA_MOCK)
                .await;
            inject_mock_schema_to_index(
                db_cfg,
                &mut initial_system_doc,
                "actors",
                ACTORS_SCHEMA_MOCK,
            )
            .await;
            inject_mock_schema_to_index(
                db_cfg,
                &mut initial_system_doc,
                "articles",
                ARTICLES_SCHEMA_MOCK,
            )
            .await;
            inject_mock_schema_to_index(
                db_cfg,
                &mut initial_system_doc,
                "configuration_items",
                CONFIG_ITEMS_SCHEMA_MOCK,
            )
            .await;
            inject_mock_schema_to_index(
                db_cfg,
                &mut initial_system_doc,
                "finance",
                FINANCE_SCHEMA_MOCK,
            )
            .await;

            inject_v2_schema_mock(db_cfg, &mut initial_system_doc, "assurance/quality_report")
                .await;
            inject_v2_schema_mock(db_cfg, &mut initial_system_doc, "assurance/xai_frame").await;
            inject_v2_schema_mock(db_cfg, &mut initial_system_doc, "common/types/base").await;

            inject_v2_schema_mock(
                db_cfg,
                &mut initial_system_doc,
                "agents/memory/vector_store_record",
            )
            .await;
            inject_v2_schema_mock(
                db_cfg,
                &mut initial_system_doc,
                "agents/memory/chat_session",
            )
            .await;

            fs::write_json_atomic_async(&sys_path, &initial_system_doc)
                .await
                .unwrap();
        }

        let uri = format!(
            "db://{}/{}/schemas/v1/db/index.schema.json",
            BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
        );
        manager.init_db_with_schema(&uri).await
    }
}

pub struct AgentDbSandbox {
    _dir: TempDir,
    pub db: SharedRef<StorageEngine>,
    pub config: AppConfig,
    pub domain_root: PathBuf,
}

impl AgentDbSandbox {
    pub async fn new() -> Self {
        let base = DbSandbox::new().await;
        let db = SharedRef::new(base.storage);
        let domain_root = base.config.get_path("PATH_RAISE_DOMAIN").unwrap();

        let temp_manager = CollectionsManager::new(
            &db,
            &base.config.mount_points.system.domain,
            &base.config.mount_points.system.db,
        );

        match DbSandbox::mock_db(&temp_manager).await {
            Ok(_) => {}
            Err(e) => panic!(
                "Erreur lors de l'initialisation de la DB dans la Sandbox : {:?}",
                e
            ),
        }

        Self {
            _dir: base._dir,
            db,
            config: base.config,
            domain_root,
        }
    }
}

pub struct GlobalDbSandbox {
    pub db: SharedRef<StorageEngine>,
    pub config: &'static AppConfig,
    pub domain_root: PathBuf,
}

impl GlobalDbSandbox {
    pub async fn new() -> Self {
        inject_mock_config().await;
        let config = AppConfig::get();
        let db_root = config.get_path("PATH_RAISE_DOMAIN").unwrap();

        let cfg_db = JsonDbConfig::new(db_root.clone());
        let storage = StorageEngine::new(cfg_db.clone());

        let manager = CollectionsManager::new(
            &storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let _ = manager.drop_db().await;

        // 🎯 NOUVELLE LOGIQUE DDL
        let sys_path = cfg_db
            .db_root(BOOTSTRAP_DOMAIN, BOOTSTRAP_DB)
            .join("_system.json");
        fs::ensure_dir_async(sys_path.parent().unwrap())
            .await
            .unwrap();
        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/index.schema.json",
            BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
        );
        let mut initial_system_doc = json_value!({
            "$schema": schema_uri,
            "name": format!("{}_{}", BOOTSTRAP_DOMAIN, BOOTSTRAP_DB),
            "space": BOOTSTRAP_DOMAIN,
            "database": BOOTSTRAP_DB,
            "schemas": { "v1": {}, "v2": {} }
        });

        let db_cfg = &manager.storage.config;
        inject_core_schemas_to_index(db_cfg, &mut initial_system_doc).await;
        inject_mock_schema_to_index(db_cfg, &mut initial_system_doc, "users", USER_SCHEMA_MOCK)
            .await;
        inject_mock_schema_to_index(
            db_cfg,
            &mut initial_system_doc,
            "actors",
            ACTORS_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(
            db_cfg,
            &mut initial_system_doc,
            "articles",
            ARTICLES_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(
            db_cfg,
            &mut initial_system_doc,
            "configuration_items",
            CONFIG_ITEMS_SCHEMA_MOCK,
        )
        .await;
        inject_mock_schema_to_index(
            db_cfg,
            &mut initial_system_doc,
            "finance",
            FINANCE_SCHEMA_MOCK,
        )
        .await;
        fs::write_json_atomic_async(&sys_path, &initial_system_doc)
            .await
            .unwrap();

        match manager.init_db().await {
            Ok(_) => {}
            Err(e) => panic!("Impossible d'initialiser la GlobalDbSandbox : {:?}", e),
        }

        Self {
            db: SharedRef::new(storage),
            config,
            domain_root: db_root,
        }
    }
}

// =========================================================================
// TESTS UNITAIRES DES MOCKS (Validation de l'infrastructure)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_inject_core_schemas_populates_json() -> RaiseResult<()> {
        let mut sys_doc = json_value!({});
        let cfg = JsonDbConfig::new(tempdir().unwrap().path().to_path_buf());
        inject_core_schemas_to_index(&cfg, &mut sys_doc).await;

        assert!(
            sys_doc["schemas"]["v1"]["db/index.schema.json"].is_object(),
            "Le schéma d'index doit être injecté"
        );
        assert!(
            sys_doc["schemas"]["v1"]["db/generic.schema.json"].is_object(),
            "Le schéma générique doit être injecté"
        );
        Ok(())
    }
    #[tokio::test]
    async fn test_inject_mock_schema_populates_json() -> RaiseResult<()> {
        let mut sys_doc = json_value!({});
        let cfg = JsonDbConfig::new(tempdir().unwrap().path().to_path_buf());
        inject_mock_schema_to_index(
            &cfg,
            &mut sys_doc,
            "test_collection",
            r#"{"type": "object"}"#,
        )
        .await;

        assert!(
            sys_doc["schemas"]["v1"]["mock/test_collection.schema.json"].is_object(),
            "Le schéma mock doit être présent"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_agent_db_sandbox_initializes_and_injects_sessions() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;

        let session_meta_path = sandbox.domain_root.join(format!(
            "{}/{}/collections/sessions/_meta.json",
            BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
        ));

        assert!(
            fs::exists_async(&session_meta_path).await,
            "Le _meta.json de session manque dans la sandbox !"
        );

        let content = match fs::read_to_string_async(&session_meta_path).await {
            Ok(c) => c,
            Err(e) => panic!("Impossible de lire _meta.json : {:?}", e),
        };

        let expected_schema_uri = format!(
            "db://{}/{}/schemas/v1/mock/sessions.schema.json",
            BOOTSTRAP_DOMAIN, BOOTSTRAP_DB
        );
        assert!(
            content.contains(&expected_schema_uri),
            "Le lien URI vers le mock de session est cassé"
        );

        Ok(())
    }
}
