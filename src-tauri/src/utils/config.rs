// FICHIER : src-tauri/src/utils/config.rs

use crate::utils::error::{AppError, Result};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Singleton global pour la configuration
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

/// Constantes Système (Single Source of Truth)
pub const SYSTEM_DOMAIN: &str = "_system";
pub const SYSTEM_DB: &str = "_system";

/// Configuration globale structurée par niveaux de responsabilité
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppConfig {
    pub name: Option<HashMap<String, String>>,

    // --- NIVEAU 1 : SYSTEME (Global) ---
    #[serde(default = "default_system_domain")]
    pub system_domain: String,
    #[serde(default = "default_system_db")]
    pub system_db: String,

    pub core: CoreConfig,

    // Gestion transparente de la conversion Liste -> Map via Serde
    #[serde(deserialize_with = "deserialize_paths_flexible")]
    pub paths: HashMap<String, String>,

    #[serde(default)]
    pub services: HashMap<String, ServiceConfig>,
    #[serde(default)]
    pub ai_engines: HashMap<String, AiEngineConfig>,
    #[serde(default)]
    pub integrations: IntegrationsConfig,

    // --- NIVEAU 2 & 3 : SURCHARGES (Contextuelles) ---
    pub workstation: Option<ScopeConfig>,
    pub user: Option<ScopeConfig>,
}

/// Configuration spécifique à un contexte (Poste ou Utilisateur)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScopeConfig {
    pub id: String,
    pub default_domain: Option<String>,
    pub default_db: Option<String>,
    pub language: Option<String>,
}

// --- HELPERS SERDE ---

fn default_system_domain() -> String {
    SYSTEM_DOMAIN.to_string()
}
fn default_system_db() -> String {
    SYSTEM_DB.to_string()
}

fn deserialize_paths_flexible<'de, D>(
    deserializer: D,
) -> std::result::Result<HashMap<String, String>, D::Error>
where
    D: Deserializer<'de>,
{
    let v: Value = Deserialize::deserialize(deserializer)?;

    if let Some(map) = v.as_object() {
        let mut paths = HashMap::new();
        for (key, val) in map {
            if let Some(s) = val.as_str() {
                paths.insert(key.clone(), s.to_string());
            }
        }
        Ok(paths)
    } else if let Some(arr) = v.as_array() {
        let mut paths = HashMap::new();
        for item in arr {
            let id = item.get("id").and_then(|v| v.as_str());
            let val = item.get("value").and_then(|v| v.as_str());
            if let (Some(k), Some(v)) = (id, val) {
                paths.insert(k.to_string(), v.to_string());
            }
        }
        Ok(paths)
    } else {
        Err(serde::de::Error::custom(
            "Format de 'paths' invalide : attendu Map ou Liste",
        ))
    }
}

// --- SOUS-STRUCTURES DE CONFIGURATION ---

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoreConfig {
    pub env_mode: String,
    pub graph_mode: String,
    pub log_level: String,
    pub vector_store_provider: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServiceConfig {
    pub status: String,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}

#[derive(Clone, Serialize, Deserialize, PartialEq)]
pub struct AiEngineConfig {
    pub status: String,
    pub provider: String,
    pub model_name: String,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub rust_repo_id: Option<String>,
    pub rust_model_file: Option<String>,
}

impl std::fmt::Debug for AiEngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AiEngineConfig")
            .field("status", &self.status)
            .field("provider", &self.provider)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct IntegrationsConfig {
    pub github_token: Option<String>,
    pub compose_profiles: Option<String>,
}

impl std::fmt::Debug for IntegrationsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationsConfig").finish()
    }
}

// --- IMPLÉMENTATION PRINCIPALE ---

impl AppConfig {
    pub fn init() -> Result<()> {
        if CONFIG.get().is_some() {
            return Ok(());
        }

        let target_env = if cfg!(test) || env::var("RAISE_ENV_MODE").as_deref() == Ok("test") {
            "test".to_string()
        } else if let Ok(env_override) = env::var("RAISE_ENV_MODE") {
            env_override
        } else if cfg!(debug_assertions) {
            "development".to_string()
        } else {
            "production".to_string()
        };

        let config = if target_env == "test" {
            Self::load_test_sandbox()?
        } else {
            Self::load_production_config(&target_env)?
        };

        CONFIG
            .set(config)
            .map_err(|_| AppError::Config("Echec initialisation OnceLock config".into()))?;

        Ok(())
    }

    pub fn get() -> &'static AppConfig {
        CONFIG
            .get()
            .expect("❌ AppConfig non initialisé ! Appelez AppConfig::init() au démarrage.")
    }

    pub fn get_path(&self, id: &str) -> Option<PathBuf> {
        self.paths.get(id).map(PathBuf::from)
    }

    /// Charge la configuration de test (sandbox)
    fn load_test_sandbox() -> Result<Self> {
        let manifest = env::var("CARGO_MANIFEST_DIR")
            .map_err(|e| AppError::Config(format!("Env CARGO_MANIFEST_DIR manquant: {}", e)))?;

        let path = PathBuf::from(manifest).join("tests/config.test.json");

        if !path.exists() {
            return Ok(Self::create_default_test_config());
        }

        let content = fs::read_to_string(&path)
            .map_err(|e| AppError::Config(format!("Erreur lecture config test: {}", e)))?;

        let mut config: AppConfig = serde_json::from_str(&content)
            .map_err(|e| AppError::Config(format!("Erreur parsing config test: {}", e)))?;

        // Isolation dynamique des chemins /tmp pour éviter les conflits de tests
        if let Some(domain_path) = config.paths.get_mut("PATH_RAISE_DOMAIN") {
            // ✅ CORRECTION COMPILATION : On rend explicite la conversion en &str
            let temp_dir = env::temp_dir();
            let temp_str = temp_dir.to_string_lossy();

            // Le cast 'as &str' lève l'ambiguïté sur AsRef
            if domain_path.starts_with("/tmp") || domain_path.starts_with(temp_str.as_ref() as &str)
            {
                let unique_id = format!(
                    "{}_{}",
                    std::process::id(),
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap()
                        .as_micros()
                );
                *domain_path = format!("{}_{}", domain_path, unique_id);
                let _ = fs::create_dir_all(domain_path);
            }
        }

        config.system_domain = SYSTEM_DOMAIN.to_string();
        config.system_db = SYSTEM_DB.to_string();

        Ok(config)
    }

    fn create_default_test_config() -> Self {
        let mut paths = HashMap::new();
        let tmp = env::temp_dir();
        paths.insert(
            "PATH_RAISE_DOMAIN".to_string(),
            tmp.to_string_lossy().to_string(),
        );
        paths.insert(
            "PATH_LOGS".to_string(),
            tmp.join("logs").to_string_lossy().to_string(),
        );

        AppConfig {
            name: Some(HashMap::from([(
                "en".to_string(),
                "Default Test Config".to_string(),
            )])),
            system_domain: SYSTEM_DOMAIN.to_string(),
            system_db: SYSTEM_DB.to_string(),
            workstation: None,
            user: None,
            core: CoreConfig {
                env_mode: "test".to_string(),
                graph_mode: "none".to_string(),
                log_level: "debug".to_string(),
                vector_store_provider: "memory".to_string(),
                language: "en".to_string(),
            },
            paths,
            services: HashMap::new(),
            ai_engines: HashMap::new(),
            integrations: IntegrationsConfig::default(),
        }
    }

    fn load_production_config(env: &str) -> Result<Self> {
        let system_json = Self::load_collection_doc("configs", |v| {
            v.get("core")
                .and_then(|c| c.get("env_mode"))
                .and_then(|e| e.as_str())
                == Some(env)
        })
        .ok_or_else(|| AppError::Config(format!("Config système introuvable pour : {}", env)))?;

        let mut config: AppConfig = serde_json::from_value(system_json)
            .map_err(|e| AppError::Config(format!("Erreur parsing System Config: {}", e)))?;

        // Charge la Workstation
        let hostname = env::var("HOSTNAME")
            .or_else(|_| env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "localhost".to_string());

        if let Some(ws_json) = Self::load_collection_doc("workstations", |v| {
            v.get("hostname").and_then(|h| h.as_str()) == Some(hostname.as_str())
        }) {
            config.workstation = Some(ScopeConfig {
                id: hostname,
                default_domain: ws_json
                    .get("default_domain")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                default_db: ws_json
                    .get("default_db")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                language: ws_json
                    .get("language")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }

        // Charge le User
        let username = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        if let Some(user_json) = Self::load_collection_doc("users", |v| {
            v.get("username").and_then(|u| u.as_str()) == Some(username.as_str())
        }) {
            config.user = Some(ScopeConfig {
                id: username,
                default_domain: user_json
                    .get("default_domain")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                default_db: user_json
                    .get("default_db")
                    .and_then(|v| v.as_str())
                    .map(String::from),
                language: user_json
                    .get("language")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }

        Ok(config)
    }

    fn load_collection_doc<F>(collection_name: &str, predicate: F) -> Option<Value>
    where
        F: Fn(&Value) -> bool,
    {
        let base_domain = dirs::home_dir()?.join("raise_domain");
        let db_root = base_domain.join(SYSTEM_DOMAIN).join(SYSTEM_DB);
        let sys_index_path = db_root.join("_system.json");
        let collection_dir = db_root.join("collections").join(collection_name);

        if let Ok(sys_content) = fs::read_to_string(&sys_index_path) {
            if let Ok(sys_index) = serde_json::from_str::<Value>(&sys_content) {
                let pointer = format!("/collections/{}/items", collection_name);
                if let Some(items) = sys_index.pointer(&pointer).and_then(|v| v.as_array()) {
                    for item in items {
                        if let Some(filename) = item.get("file").and_then(|f| f.as_str()) {
                            let path = collection_dir.join(filename);
                            if path.exists() {
                                if let Ok(content) = fs::read_to_string(&path) {
                                    if let Ok(doc) = serde_json::from_str::<Value>(&content) {
                                        if predicate(&doc) {
                                            return Some(doc);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        None
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_scope_config_structure() {
        let scope = ScopeConfig {
            id: "dev-machine".to_string(),
            default_domain: Some("dev_domain".to_string()),
            default_db: Some("dev_db".to_string()),
            language: Some("fr".to_string()),
        };
        assert_eq!(scope.id, "dev-machine");
        assert_eq!(scope.default_domain.as_deref(), Some("dev_domain"));
    }

    #[test]
    fn test_deserialize_app_config_with_scopes() {
        let json_data = json!({
            "name": null,
            "system_domain": "_system",
            "system_db": "_system",
            "core": {
                "env_mode": "test",
                "graph_mode": "none",
                "log_level": "debug",
                "vector_store_provider": "memory",
                "language": "en"
            },
            "paths": { "PATH_TEST": "/tmp" },
            "services": {},
            "ai_engines": {},
            "integrations": {},
            "workstation": {
                "id": "host1",
                "default_domain": "ws_domain"
            },
            "user": {
                "id": "user1",
                "default_db": "user_db"
            }
        });

        let config: AppConfig = serde_json::from_value(json_data).expect("Désérialisation échouée");

        assert_eq!(config.system_domain, "_system");

        let ws = config.workstation.expect("Workstation should be present");
        assert_eq!(ws.id, "host1");
        assert_eq!(ws.default_domain.as_deref(), Some("ws_domain"));

        let user = config.user.expect("User should be present");
        assert_eq!(user.id, "user1");
        assert_eq!(user.default_db.as_deref(), Some("user_db"));
    }

    #[test]
    fn test_deserialize_paths_list_compat() {
        let json_data = json!({
            "system_domain": "_sys",
            "system_db": "_db",
            "core": {
                "env_mode": "test",
                "graph_mode": "none",
                "log_level": "debug",
                "vector_store_provider": "memory",
                "language": "en"
            },
            "paths": [
                { "id": "P1", "value": "/v1" }
            ],
            "services": {}, "ai_engines": {}, "integrations": {}
        });

        let config: AppConfig = serde_json::from_value(json_data).unwrap();
        assert_eq!(config.paths.get("P1").unwrap(), "/v1");
    }
}

// --- MODULE MOCKS PUBLIC (Pour integration tests) ---
pub mod test_mocks {
    use super::*;

    pub fn inject_mock_config() {
        if CONFIG.get().is_some() {
            return;
        }

        let config = AppConfig::create_default_test_config();

        if let Some(path) = config.paths.get("PATH_RAISE_DOMAIN") {
            let _ = fs::create_dir_all(path);
        }

        let _ = CONFIG.set(config);
    }
}
