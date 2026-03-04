// FICHIER : src-tauri/src/utils/mock.rs
#![cfg(any(test, debug_assertions))]

// 🎯 100% FAÇADE UTILS : On respecte le contrat de mod.rs
use crate::utils::io::{create_dir_all, exists, tempdir, write, Path, PathBuf, TempDir};
use crate::utils::prelude::*;
use crate::utils::Arc;

// Accès au Singleton pour l'injection
use crate::utils::config::CONFIG;

// Dépendances métier (hors utils)
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
    settings: Value,
) {
    let _ = manager
        .create_collection(
            "components",
            "db://_system/_system/schemas/v1/db/generic.schema.json",
        )
        .await;

    let doc = json!({
        "id": format!("mock-{}", comp_id),
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
    let schema_file = schema_dir.join("index.schema.json");

    if !exists(&schema_file).await {
        let _ = create_dir_all(&schema_dir).await;

        let core_schema = json!({
            "$id": "db://_system/_system/schemas/v1/db/index.schema.json",
            "type": "object",
            "properties": {
                "name": { "type": "string" },
                "version": { "type": "integer", "default": 1 },
                "space": { "type": "string" },
                "database": { "type": "string" },
                "collections": { "type": "object", "default": {} },
                "rules": { "type": "object", "default": {} },
                "schemas": { "type": "object", "default": { "v1": {} } }
            },
            "required": ["name", "version", "space", "database", "collections", "rules", "schemas"]
        });

        let _ = write(&schema_file, core_schema.to_string().as_bytes()).await;

        // Schéma générique (permissif pour débloquer les tests)
        let generic_schema = json!({
            "$id": "db://_system/_system/schemas/v1/db/generic.schema.json",
            "type": "object",
            "additionalProperties": true
        });
        let _ = write(
            &schema_dir.join("generic.schema.json"),
            generic_schema.to_string().as_bytes(),
        )
        .await;

        // Schéma interne des migrations
        let migration_schema = json!({
            "$id": "db://_system/_system/schemas/v1/db/migration.schema.json",
            "type": "object",
            "additionalProperties": true
        });
        let _ = write(
            &schema_dir.join("migration.schema.json"),
            migration_schema.to_string().as_bytes(),
        )
        .await;
    }
}

pub async fn inject_collection_schema(domain_root: &Path, collection_name: &str, content: &str) {
    let schemas_dir = domain_root.join("_system/_system/schemas/v1/mock");
    let _ = create_dir_all(&schemas_dir).await;

    let schema_uri = format!(
        "db://_system/_system/schemas/v1/mock/{}.schema.json",
        collection_name
    );
    let schema_file = schemas_dir.join(format!("{}.schema.json", collection_name));

    // On s'assure que le contenu JSON a bien le bon $id
    let mut json_val: Value = serde_json::from_str(content).unwrap_or(json!({}));
    if let Some(obj) = json_val.as_object_mut() {
        obj.insert("$id".to_string(), Value::String(schema_uri.clone()));
    }
    let _ = write(&schema_file, json_val.to_string().as_bytes()).await;

    // 2. Créer le dossier de la collection ET son _meta.json indispensable !
    let col_dir = domain_root
        .join("_system/_system/collections")
        .join(collection_name);
    let _ = create_dir_all(&col_dir).await;

    let meta_content = json!({
        "schema": schema_uri,
        "indexes": []
    });

    let _ = write(
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
    pub db: Arc<StorageEngine>,
    pub config: AppConfig,
    pub domain_root: PathBuf,
}

impl AgentDbSandbox {
    pub async fn new() -> Self {
        let base = DbSandbox::new().await;
        let db = Arc::new(base.storage);
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
    pub db: Arc<StorageEngine>,
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
            db: Arc::new(storage),
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
    use crate::utils::io::read_to_string;

    #[tokio::test]
    async fn test_inject_schema_to_path_creates_valid_file() {
        let dir = tempdir().unwrap();
        let db_cfg = JsonDbConfig::new(dir.path().to_path_buf());

        inject_schema_to_path(&db_cfg).await;

        let schema_file = db_cfg
            .db_schemas_root("_system", "_system")
            .join("v1/db/index.schema.json");
        assert!(
            exists(&schema_file).await,
            "Le fichier index.schema.json n'a pas été créé"
        );
    }

    #[tokio::test]
    async fn test_inject_collection_schema_writes_correctly() {
        let dir = tempdir().unwrap();
        let root_path = dir.path().to_path_buf();

        let mock_schema = r#"{"type": "object"}"#;
        inject_collection_schema(&root_path, "test_collection", mock_schema).await;

        // 🎯 Vérification du _meta.json vital
        let meta_file = root_path.join("_system/_system/collections/test_collection/_meta.json");
        assert!(exists(&meta_file).await, "_meta.json n'a pas été créé");

        // 🎯 Vérification du placement officiel du schéma
        let schema_file =
            root_path.join("_system/_system/schemas/v1/mock/test_collection.schema.json");
        assert!(
            exists(&schema_file).await,
            "Le schéma n'a pas été placé dans le registre"
        );
    }

    #[tokio::test]
    async fn test_agent_db_sandbox_initializes_and_injects_sessions() {
        let sandbox = AgentDbSandbox::new().await;

        // La collection session doit avoir son fichier _meta.json !
        let session_meta_path = sandbox
            .domain_root
            .join("_system/_system/collections/sessions/_meta.json");
        assert!(
            exists(&session_meta_path).await,
            "Le _meta.json de session manque dans la sandbox !"
        );

        let content = read_to_string(&session_meta_path).await.unwrap();
        assert!(
            content.contains("db://_system/_system/schemas/v1/mock/sessions.schema.json"),
            "Le lien URI vers le mock de session est cassé"
        );
    }
}
