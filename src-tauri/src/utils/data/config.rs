// FICHIER : src-tauri/src/utils/data/config.rs

// 1. Base de données (AI-Ready Queries)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};

// 2. Core : Environnement, Concurrence et Erreurs
use crate::raise_error;
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{RuntimeEnv, StaticCell}; // Macro d'erreur globale

// 3. I/O : Système de fichiers
use crate::utils::io::fs::{self, PathBuf};

// 4. Data : Traits, Collections sémantiques et JSON
use crate::utils::data::json::{self, json_value, JsonValue};
use crate::utils::data::{
    CustomDeserializerEngine, Deserializable, DeserializationErrorTrait, Serializable, UnorderedMap,
};

/// Singleton global pour la configuration
pub static CONFIG: StaticCell<AppConfig> = StaticCell::new();
/// Constantes Système (Single Source of Truth)
pub const SYSTEM_DOMAIN: &str = "_system";
pub const SYSTEM_DB: &str = "_system";

/// Configuration globale structurée par niveaux de responsabilité
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct AppConfig {
    pub name: Option<UnorderedMap<String, String>>,

    // --- NIVEAU 1 : SYSTEME (Global) ---
    #[serde(default = "fallback_system_domain")]
    pub system_domain: String,

    #[serde(default = "fallback_system_db")]
    pub system_db: String,

    pub core: CoreConfig,

    #[serde(default = "fallback_world_model")]
    pub world_model: WorldModelConfig,

    #[serde(default = "fallback_deep_learning")]
    pub deep_learning: DeepLearningConfig,

    // Gestion transparente de la conversion Liste -> Map via Serde
    #[serde(deserialize_with = "deserialize_paths_flexible")]
    pub paths: UnorderedMap<String, String>,

    // 🎯 Pointeurs UUID vers la Base de Données
    pub active_dapp: String,

    #[serde(default = "fallback_empty_services")]
    pub active_services: Vec<String>,

    #[serde(default = "fallback_empty_components")]
    pub active_components: Vec<String>,

    #[serde(default = "fallback_integrations")]
    pub integrations: IntegrationsConfig,

    // --- NIVEAU 2 & 3 : SURCHARGES (Contextuelles) ---
    pub workstation: Option<ScopeConfig>,
    pub user: Option<ScopeConfig>,
}

/// Configuration spécifique à un contexte (Poste ou Utilisateur)
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct ScopeConfig {
    pub id: String,
    pub default_domain: Option<String>,
    pub default_db: Option<String>,
    pub language: Option<String>,
}

// =========================================================================
// 🤖 FALLBACKS EXPLICITES POUR LA DÉSÉRIALISATION (AI-Ready)
// =========================================================================

fn fallback_system_domain() -> String {
    SYSTEM_DOMAIN.to_string()
}
fn fallback_system_db() -> String {
    SYSTEM_DB.to_string()
}
fn fallback_world_model() -> WorldModelConfig {
    WorldModelConfig::default()
}
fn fallback_deep_learning() -> DeepLearningConfig {
    DeepLearningConfig::default()
}
fn fallback_integrations() -> IntegrationsConfig {
    IntegrationsConfig::default()
}

/// Fallback si la liste des services est absente du JSON
fn fallback_empty_services() -> Vec<String> {
    Vec::new()
}

/// Fallback si la liste des composants est absente du JSON
fn fallback_empty_components() -> Vec<String> {
    Vec::new()
}

// =========================================================================
// 🛠️ DÉSÉRIALISATION CUSTOMISÉE
// =========================================================================

fn deserialize_paths_flexible<'de, D>(
    deserializer: D,
) -> std::result::Result<UnorderedMap<String, String>, D::Error>
where
    D: CustomDeserializerEngine<'de>,
{
    // 🎯 On utilise notre alias JsonValue
    let v: JsonValue = Deserializable::deserialize(deserializer)?;

    if let Some(map) = v.as_object() {
        let mut paths = UnorderedMap::new();
        for (key, val) in map {
            if let Some(s) = val.as_str() {
                paths.insert(key.clone(), s.to_string());
            }
        }
        Ok(paths)
    } else if let Some(arr) = v.as_array() {
        let mut paths = UnorderedMap::new();
        for item in arr {
            let id = item.get("id").and_then(|v| v.as_str());
            let val = item.get("value").and_then(|v| v.as_str());
            if let (Some(k), Some(v)) = (id, val) {
                paths.insert(k.to_string(), v.to_string());
            }
        }
        Ok(paths)
    } else {
        Err(DeserializationErrorTrait::custom(
            "Format de 'paths' invalide : attendu JsonObject ou Liste",
        ))
    }
}

// =========================================================================
// SOUS-STRUCTURES DE CONFIGURATION
// =========================================================================

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct CoreConfig {
    pub env_mode: String,
    pub graph_mode: String,
    pub log_level: String,
    pub vector_store_provider: String,
    pub language: String,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct WorldModelConfig {
    pub vocab_size: usize,
    pub embedding_dim: usize,
    pub action_dim: usize,
    pub hidden_dim: usize,
    pub use_gpu: bool,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct DeepLearningConfig {
    pub input_size: usize,
    pub hidden_size: usize,
    pub output_size: usize,
    pub learning_rate: f64,
    pub device: String,
}

#[derive(Clone, Serializable, Deserializable, PartialEq, Default)]
pub struct IntegrationsConfig {
    pub github_token: Option<String>,
    pub compose_profiles: Option<String>,
}

impl std::fmt::Debug for IntegrationsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IntegrationsConfig").finish()
    }
}

// =========================================================================
// IMPLÉMENTATION PRINCIPALE
// =========================================================================

impl AppConfig {
    pub fn init() -> RaiseResult<()> {
        if CONFIG.get().is_some() {
            return Ok(());
        }

        let target_env = if cfg!(test) || RuntimeEnv::var("RAISE_ENV_MODE").as_deref() == Ok("test")
        {
            "test".to_string()
        } else if let Ok(env_override) = RuntimeEnv::var("RAISE_ENV_MODE") {
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

        if CONFIG.set(config).is_err() {
            raise_error!(
                "ERR_CONFIG_INIT_ONCE",
                error = "La configuration est déjà initialisée"
            );
        }

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

    pub async fn get_component_settings(
        manager: &CollectionsManager<'_>,
        component_id: &str,
    ) -> RaiseResult<JsonValue> {
        let mut query = Query::new("components");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq(
                "identity.component_id",
                JsonValue::String(component_id.to_string()), // 🎯 Remplacé
            )],
        });

        let result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_CONFIG_DB_QUERY",
                error = e,
                context = json_value!({ "requested_id": component_id })
            ),
        };

        let Some(comp_doc) = result.documents.first() else {
            raise_error!(
                "ERR_CONFIG_COMPONENT_MISSING",
                error = "Composant introuvable en base de données",
                context = json_value!({ "requested_id": component_id })
            );
        };

        let Some(settings) = comp_doc.get("settings").cloned() else {
            raise_error!(
                "ERR_CONFIG_SETTINGS_MISSING",
                error = "Champ 'settings' manquant dans le document",
                context = json_value!({ "requested_id": component_id })
            );
        };

        Ok(settings)
    }

    fn load_test_sandbox() -> RaiseResult<Self> {
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
            return Ok(Self::create_default_test_config());
        }

        let content = match fs::read_to_string_sync(&path) {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_CONFIG_FS_READ",
                error = e,
                context = json_value!({ "path": path.to_string_lossy() })
            ),
        };

        // 🎯 Utilisation de notre fonction sémantique de façade
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

        config.system_domain = SYSTEM_DOMAIN.to_string();
        config.system_db = SYSTEM_DB.to_string();

        Ok(config)
    }

    pub(crate) fn create_default_test_config() -> Self {
        // 🎯 Utilisation de notre UnorderedMap
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
            name: Some(UnorderedMap::from([(
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
            world_model: WorldModelConfig::default(),
            deep_learning: DeepLearningConfig::default(),
            paths,
            active_dapp: "mock-dapp-id".to_string(),
            active_services: vec!["mock-service-id".to_string()],
            active_components: vec!["mock-comp-id-1".to_string()],
            integrations: IntegrationsConfig::default(),
        }
    }

    fn load_production_config(env: &str) -> RaiseResult<Self> {
        let system_json = Self::load_collection_doc("configs", |v| {
            v.get("core")
                .and_then(|c| c.get("env_mode"))
                .and_then(|e| e.as_str())
                == Some(env)
        });

        let Some(json_val) = system_json else {
            raise_error!(
                "ERR_CONFIG_SYS_MISSING",
                error = "Configuration système introuvable",
                context = json_value!({ "target_environment": env })
            );
        };

        // 🎯 Utilisation de notre fonction sémantique de façade
        let mut config: AppConfig = match json::deserialize_from_value(json_val) {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_CONFIG_DESERIALIZE",
                error = e,
                context = json_value!({ "env": env })
            ),
        };

        let hostname = RuntimeEnv::var("HOSTNAME")
            .or_else(|_| RuntimeEnv::var("COMPUTERNAME"))
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

        let username = RuntimeEnv::var("USER")
            .or_else(|_| RuntimeEnv::var("USERNAME"))
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

    fn load_collection_doc<F>(collection_name: &str, predicate: F) -> Option<JsonValue>
    where
        F: Fn(&JsonValue) -> bool,
    {
        let base_domain = dirs::home_dir()?.join("raise_domain");
        let db_root = base_domain.join(SYSTEM_DOMAIN).join(SYSTEM_DB);
        let sys_index_path = db_root.join("_system.json");
        let collection_dir = db_root.join("collections").join(collection_name);

        let sys_content = fs::read_to_string_sync(&sys_index_path).ok()?;

        // 🎯 Remplacement de serde_json::from_str
        let sys_index: JsonValue = json::deserialize_from_str(&sys_content).ok()?;

        let pointer = format!("/collections/{}/items", collection_name);
        let items = sys_index.pointer(&pointer)?.as_array()?;

        for item in items {
            let filename = item.get("file").and_then(|f| f.as_str())?;
            let path = collection_dir.join(filename);

            if let Ok(content) = fs::read_to_string_sync(&path) {
                if let Ok(doc) = json::deserialize_from_str::<JsonValue>(&content) {
                    if predicate(&doc) {
                        return Some(doc);
                    }
                }
            }
        }
        None
    }
}

// =========================================================================
// IMPLÉMENTATIONS PAR DÉFAUT
// =========================================================================

impl Default for WorldModelConfig {
    fn default() -> Self {
        Self {
            vocab_size: 10,
            embedding_dim: 16,
            action_dim: 5,
            hidden_dim: 32,
            use_gpu: cfg!(feature = "cuda"),
        }
    }
}

impl Default for DeepLearningConfig {
    fn default() -> Self {
        Self {
            input_size: 10,
            hidden_size: 20,
            output_size: 5,
            learning_rate: 0.01,
            device: if cfg!(feature = "cuda") {
                "cuda".into()
            } else {
                "cpu".into()
            },
        }
    }
}

impl DeepLearningConfig {
    pub fn to_device(&self) -> candle_core::Device {
        match self.device.as_str() {
            "cuda" => candle_core::Device::new_cuda(0).unwrap_or(candle_core::Device::Cpu),
            _ => candle_core::Device::Cpu,
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

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
        let json_data = json_value!({
            "name": null,
            "system_domain": "_system",
            "system_db": "_system",
            "active_dapp": "mock-dapp",
            "core": {
                "env_mode": "test",
                "graph_mode": "none",
                "log_level": "debug",
                "vector_store_provider": "memory",
                "language": "en"
            },
            "paths": { "PATH_TEST": "/tmp" },
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

        let config: AppConfig =
            json::deserialize_from_value(json_data).expect("Désérialisation échouée");
        assert_eq!(config.system_domain, "_system");
        assert_eq!(config.workstation.unwrap().id, "host1");
        assert_eq!(config.user.unwrap().id, "user1");
    }

    #[test]
    fn test_deserialize_paths_list_compat() {
        let json_data = json_value!({
            "system_domain": "_sys",
            "system_db": "_db",
            "active_dapp": "mock-dapp",
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
            "integrations": {}
        });

        let config: AppConfig = json::deserialize_from_value(json_data).unwrap();
        assert_eq!(config.paths.get("P1").unwrap(), "/v1");
    }
}
