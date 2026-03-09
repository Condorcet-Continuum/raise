// FICHIER : src-tauri/src/utils/testing/mock.rs
#![cfg(any(test, debug_assertions))]

// 1. Core : Concurrence, Mémoire et Identifiants
use crate::utils::core::SharedRef;
use crate::utils::io::fs::{self, tempdir, Path, PathBuf, TempDir};

// 2. Data : Configuration, JSON et Traits
use crate::utils::data::config::{AppConfig, CONFIG};
use crate::utils::data::json::{self, json_value, JsonValue};

// 4. Dépendances métier (Base de données JSON)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::{JsonDbConfig, StorageEngine};

// --- DÉFINITION DES SCHÉMAS STANDARDS POUR TESTS ---

pub const SESSION_SCHEMA_MOCK: &str = r#"{
    "type": "object",
    "properties": {
        "user_id": { "type": "string", "format": "uuid" },
        "user_name": { "type": "string" },
        "status": { "type": "string", "enum": ["active", "idle", "expired", "revoked"] },
        "expires_at": { "type": "string", "format": "date-time" },
        "last_activity_at": { "type": "string", "format": "date-time" },
        "context": { 
            "type": "object",
            "required": ["current_domain", "current_db", "active_dapp"]
        }
    },
    "required": ["user_id", "status", "expires_at", "context"]
}"#;

// --- FONCTIONS MOCKS ---

pub async fn inject_mock_component(
    manager: &CollectionsManager<'_>,
    comp_id: &str,
    settings: JsonValue, // 🎯 JsonValue au lieu de Value
) {
    let _ = manager
        .create_collection(
            "components",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;

    // 🎯 json_value! au lieu de json!
    let doc = json_value!({
        "_id": format!("mock-{}", comp_id),
        "identity": { "component_id": comp_id },
        "settings": settings,
        "$schema": "db://_system/_system/schemas/v1/db/generic.schema.json"
    });

    manager
        .insert_raw("components", &doc)
        .await
        .expect("Échec de l'injection du composant Mock à cause du schéma strict");
}

/// Injecte le schéma racine index.schema.json et les passe-partouts
pub async fn inject_schema_to_path(db_cfg: &JsonDbConfig) {
    let schema_dir = db_cfg.db_schemas_root("_system", "_system").join("v1/db");

    // 🎯 Utilisation de notre façade asynchrone FS
    let _ = fs::create_dir_all_async(&schema_dir).await;

    // 1. Schéma de migration (🎯 json_value!)
    let migration_schema = json_value!({
        "$id": "db://_system/_system/schemas/v1/db/migration.schema.json",
        "type": "object",
        "properties": {
            "_id": { "type": "string" },
            "version": { "type": "string" }
        },
        "required": ["_id", "version"]
    });

    // 🎯 Façade fs
    let _ =
        fs::write_json_atomic_async(&schema_dir.join("migration.schema.json"), &migration_schema)
            .await;

    // 2. Schéma d'index avec 'properties' explicites pour l'hydratation
    let core_schema = json_value!({
        "$id": "db://_system/_system/schemas/v1/db/index.schema.json",
        // ... (Reste de la définition du schéma inchangée mais formatée proprement) ...
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
                            "schema": "db://_system/_system/schemas/v1/db/migration.schema.json",
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
                            "schema": "db://_system/_system/schemas/v1/db/rule.schema.json",
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

    // Schéma générique
    let generic_schema = json_value!({
        "$id": "db://_system/_system/schemas/v1/db/generic.schema.json",
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
    let schemas_dir = domain_root.join("_system/_system/schemas/v1/mock");
    let _ = fs::create_dir_all_async(&schemas_dir).await;

    let schema_uri = format!(
        "db://_system/_system/schemas/v1/mock/{}.schema.json",
        collection_name
    );
    let schema_file = schemas_dir.join(format!("{}.schema.json", collection_name));

    // 🎯 Remplacement de serde_json par json::from_str (Façade data)
    let mut json_val: JsonValue = json::deserialize_from_str(content).unwrap_or(json_value!({}));

    if let Some(obj) = json_val.as_object_mut() {
        obj.insert("$id".to_string(), JsonValue::String(schema_uri.clone())); // 🎯 JsonValue::String
    }

    // 🎯 Remplacement de tokio::fs::write par notre façade fs
    let _ = fs::write_async(&schema_file, json_val.to_string().as_bytes()).await;

    // 2. Créer le dossier de la collection ET son _meta.json indispensable !
    let col_dir = domain_root
        .join("_system/_system/collections")
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
        let config = AppConfig::create_default_test_config();
        let _ = CONFIG.set(config);
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

        let dir = tempdir().expect("Création du dossier temporaire échouée");
        let root_path = dir.path().to_path_buf();

        config.paths.insert(
            "PATH_RAISE_DOMAIN".to_string(),
            root_path.to_string_lossy().to_string(),
        );

        let db_cfg = JsonDbConfig::new(root_path.clone());

        inject_schema_to_path(&db_cfg).await;
        inject_collection_schema(&root_path, "sessions", SESSION_SCHEMA_MOCK).await;

        let storage = StorageEngine::new(db_cfg);
        Self {
            _dir: dir,
            storage,
            config,
        }
    }
}

pub struct AgentDbSandbox {
    _dir: TempDir,
    pub db: SharedRef<StorageEngine>, // 🎯 SharedRef remplace Arc
    pub config: AppConfig,
    pub domain_root: PathBuf,
}

impl AgentDbSandbox {
    pub async fn new() -> Self {
        let base = DbSandbox::new().await;
        let db = SharedRef::new(base.storage); // 🎯 SharedRef
        let domain_root = base.config.get_path("PATH_RAISE_DOMAIN").unwrap();

        let temp_manager =
            CollectionsManager::new(&db, &base.config.system_domain, &base.config.system_db);
        temp_manager
            .init_db()
            .await
            .expect("Erreur lors de l'initialisation de la DB dans la Sandbox");

        Self {
            _dir: base._dir,
            db,
            config: base.config,
            domain_root,
        }
    }
}

pub struct GlobalDbSandbox {
    pub db: SharedRef<StorageEngine>, // 🎯 SharedRef
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
        let manager = CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

        let _ = manager.drop_db().await;
        inject_schema_to_path(&cfg_db).await;
        inject_collection_schema(&db_root, "sessions", SESSION_SCHEMA_MOCK).await;

        manager
            .init_db()
            .await
            .expect("Impossible d'initialiser la GlobalDbSandbox");

        Self {
            db: SharedRef::new(storage), // 🎯 SharedRef
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
    // 🎯 Plus besoin d'importer explicitement exists ou read_to_string,

    #[tokio::test]
    async fn test_inject_schema_to_path_creates_valid_file() {
        let dir = tempdir().unwrap();
        let db_cfg = JsonDbConfig::new(dir.path().to_path_buf());

        inject_schema_to_path(&db_cfg).await;

        let schema_file = db_cfg
            .db_schemas_root("_system", "_system")
            .join("v1/db/index.schema.json");

        // 🎯 Façade FS
        assert!(
            fs::exists_async(&schema_file).await,
            "Le fichier index.schema.json n'a pas été créé"
        );
    }

    #[tokio::test]
    async fn test_inject_collection_schema_writes_correctly() {
        let dir = tempdir().unwrap();
        let root_path = dir.path().to_path_buf();

        let mock_schema = r#"{"type": "object"}"#;
        inject_collection_schema(&root_path, "test_collection", mock_schema).await;

        let meta_file = root_path.join("_system/_system/collections/test_collection/_meta.json");
        // 🎯 Façade FS
        assert!(
            fs::exists_async(&meta_file).await,
            "_meta.json n'a pas été créé"
        );

        let schema_file =
            root_path.join("_system/_system/schemas/v1/mock/test_collection.schema.json");
        // 🎯 Façade FS
        assert!(
            fs::exists_async(&schema_file).await,
            "Le schéma n'a pas été placé dans le registre"
        );
    }

    #[tokio::test]
    async fn test_agent_db_sandbox_initializes_and_injects_sessions() {
        let sandbox = AgentDbSandbox::new().await;

        let session_meta_path = sandbox
            .domain_root
            .join("_system/_system/collections/sessions/_meta.json");

        // 🎯 Façade FS
        assert!(
            fs::exists_async(&session_meta_path).await,
            "Le _meta.json de session manque dans la sandbox !"
        );

        // 🎯 Façade FS
        let content = fs::read_to_string_async(&session_meta_path).await.unwrap();
        assert!(
            content.contains("db://_system/_system/schemas/v1/mock/sessions.schema.json"),
            "Le lien URI vers le mock de session est cassé"
        );
    }
}
