// FICHIER : src-tauri/src/utils/data/config.rs

// 1. Base de données (AI-Ready Queries)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
// 2. Core : Environnement, Concurrence et Erreurs
use crate::raise_error;
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{RuntimeEnv, StaticCell, UniqueId, UtcClock};

// 3. I/O : Système de fichiers
use crate::utils::io::fs::{self, PathBuf};

// 4. Data : Traits, Collections sémantiques et JSON
use crate::utils::data::json::{self, json_value, JsonValue};
use crate::utils::data::{
    CustomDeserializerEngine, Deserializable, DeserializationErrorTrait, Serializable, UnorderedMap,
};

/// Singleton global pour la configuration
pub static CONFIG: StaticCell<AppConfig> = StaticCell::new();
pub static DEVICE: StaticCell<candle_core::Device> = StaticCell::new();

/// Constantes Système pour amorcer la première lecture
pub const BOOTSTRAP_DOMAIN: &str = "_system";
pub const BOOTSTRAP_DB: &str = "bootstrap";

/// Configuration globale structurée par niveaux de responsabilité
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct AppConfig {
    // --- MÉTADONNÉES SYSTÈMES & SÉMANTIQUES ---
    #[serde(rename = "_id", default = "fallback_id")]
    pub id: String,

    #[serde(rename = "_created_at", default = "fallback_date")]
    pub created_at: String,

    #[serde(rename = "_updated_at", default = "fallback_date")]
    pub updated_at: String,

    #[serde(rename = "@type", default = "fallback_config_type")]
    pub semantic_type: Vec<String>,

    pub name: Option<UnorderedMap<String, String>>,

    // POINTS DE MONTAGE EXPLICITES ---
    #[serde(default = "fallback_mount_points")]
    pub mount_points: MountPointsConfig,

    pub core: CoreConfig,

    #[serde(default = "fallback_world_model")]
    pub world_model: WorldModelConfig,

    #[serde(default = "fallback_deep_learning")]
    pub deep_learning: DeepLearningConfig,

    #[serde(deserialize_with = "deserialize_paths_flexible")]
    pub paths: UnorderedMap<String, String>,

    // 🎯 Pointeurs Sémantiques (Doivent stocker des valeurs du type "ref:dapps:handle:...")
    pub active_dapp_id: String, // 👈 Renommé (était active_dapp)
    pub workstation_id: String,

    #[serde(default = "fallback_empty_services")]
    pub active_services: Vec<String>,

    #[serde(default = "fallback_empty_components")]
    pub active_components: Vec<String>,

    #[serde(default = "fallback_integrations")]
    pub integrations: IntegrationsConfig,

    #[serde(default = "fallback_simulation_context")]
    pub simulation_context: SimulationContextConfig,

    // --- NIVEAU 2 & 3 : IDENTITÉS (Sans logique de routage DB) ---
    #[serde(skip)]
    pub workstation: Option<ScopeConfig>,
    #[serde(skip)]
    pub user: Option<ScopeConfig>,
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct MountPointsConfig {
    pub system: DbPointer,
    pub raise: DbPointer,
    pub exploration: DbPointer, // Incubation
    pub modeling: DbPointer,    // As-Designed
    pub simulation: DbPointer,  // As-Simulated
    pub integration: DbPointer, // V&V Physique
    pub production: DbPointer,  // As-Built
    pub operation: DbPointer,   // As-Operated
}

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct DbPointer {
    pub domain: String,
    pub db: String,
}

/// Configuration spécifique à un contexte identitaire
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct ScopeConfig {
    pub id: String,
    pub language: Option<String>,
}

// =========================================================================
// 🤖 FALLBACKS EXPLICITES POUR LA DÉSÉRIALISATION
// =========================================================================
fn fallback_id() -> String {
    UniqueId::new_v4().to_string()
}
fn fallback_date() -> String {
    UtcClock::now().to_rfc3339()
}
fn fallback_config_type() -> Vec<String> {
    vec!["SystemConfig".to_string(), "cfg:SystemConfig".to_string()]
}
fn fallback_mount_points() -> MountPointsConfig {
    MountPointsConfig {
        system: DbPointer {
            domain: "_system".into(),
            db: "_system".into(),
        },
        raise: DbPointer {
            domain: "_system".into(),
            db: "raise_core".into(),
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
    }
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
fn fallback_empty_services() -> Vec<String> {
    Vec::new()
}
fn fallback_empty_components() -> Vec<String> {
    Vec::new()
}
fn fallback_simulation_context() -> SimulationContextConfig {
    SimulationContextConfig::default()
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

#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct SimulationContextConfig {
    pub source_domain: String,
    pub source_db: String,
    pub target_domain: String,
    pub target_db: String,
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

        #[cfg(any(test, debug_assertions))]
        let config = if target_env == "test" {
            crate::utils::testing::mock::load_test_sandbox()?
        } else {
            Self::load_production_config(&target_env)?
        };

        #[cfg(not(any(test, debug_assertions)))]
        let config = Self::load_production_config(&target_env)?;

        if DEVICE.get().is_none() {
            let device = Self::detect_best_device(&config);
            let _ = DEVICE.set(device);
        }

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

    pub fn is_test_env(&self) -> bool {
        self.core.env_mode == "test"
    }

    pub fn get_path(&self, id: &str) -> Option<PathBuf> {
        self.paths.get(id).map(PathBuf::from)
    }

    pub async fn get_component_settings(
        manager: &CollectionsManager<'_>,
        component_handle: &str,
    ) -> RaiseResult<JsonValue> {
        let ref_id = format!("ref:components:handle:{}", component_handle);
        let query = Query::new("service_configs");

        let result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_CONFIG_DB_QUERY",
                error = e,
                context = json_value!({ "requested_handle": component_handle })
            ),
        };

        for doc in result.documents {
            // 🎯 FIX ABSOLU : On aligne la lecture sur le schéma V2 "service_settings"
            if let Some(svc_settings) = doc.get("service_settings").and_then(|v| v.as_object()) {
                if let Some(settings) = svc_settings.get(&ref_id) {
                    return Ok(settings.clone());
                }
            }
        }

        raise_error!(
            "ERR_CONFIG_COMPONENT_MISSING",
            error = "Configuration du composant introuvable dans les 'service_configs'",
            context = json_value!({
                "requested_handle": component_handle,
                "expected_ref": ref_id
            })
        );
    }

    pub async fn get_llm_settings(
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<(String, String)> {
        let settings = match Self::get_component_settings(manager, "ai_llm").await {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_CONFIG_LLM_FETCH_FAILED",
                error = e,
                context =
                    json_value!({ "action": "get_component_settings", "component": "ai_llm" })
            ),
        };

        let model = match settings["rust_model_file"].as_str() {
            Some(m) => m.to_string(),
            None => raise_error!(
                "ERR_CONFIG_LLM_MODEL_MISSING",
                error = "La clé 'rust_model_file' est introuvable.",
                context = json_value!({ "component": "ai_llm", "settings_dump": settings })
            ),
        };

        let tokenizer = match settings["rust_tokenizer_file"].as_str() {
            Some(t) => t.to_string(),
            None => raise_error!(
                "ERR_CONFIG_LLM_TOKENIZER_MISSING",
                error = "La clé 'rust_tokenizer_file' est introuvable.",
                context = json_value!({ "component": "ai_llm", "settings_dump": settings })
            ),
        };

        Ok((model, tokenizer))
    }

    pub async fn get_service_settings(
        manager: &CollectionsManager<'_>,
        target_service_id: &str, // ex: "ref:services:blueprint:google_gemini"
    ) -> RaiseResult<JsonValue> {
        // 1. Construction stricte de la requête via l'API QueryEngine
        let mut query = Query::new("service_configs");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq(
                "service_id",
                crate::utils::prelude::json_value!(target_service_id),
            )],
        });
        query.limit = Some(1);

        // 2. Exécution via le CollectionsManager
        let result = match QueryEngine::new(manager).execute_query(query).await {
            Ok(res) => res,
            Err(e) => raise_error!(
                "ERR_CONFIG_DB_QUERY",
                error = e,
                context = json_value!({ "requested_service": target_service_id })
            ),
        };

        // 3. Extraction sécurisée
        if let Some(doc) = result.documents.into_iter().next() {
            if let Some(settings) = doc.get("service_settings") {
                return Ok(settings.clone());
            }
        }

        raise_error!(
            "ERR_CONFIG_SERVICE_MISSING",
            error = "Configuration du service introuvable ou propriété 'service_settings' absente.",
            context = json_value!({ "requested_service": target_service_id })
        );
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

        // 🎯 On peuple le champ 'workstation' (ScopeConfig) à partir de la DB
        if let Some(ws_json) = Self::load_collection_doc("workstations", |v| {
            v.get("hostname").and_then(|h| h.as_str()) == Some(hostname.as_str())
        }) {
            config.workstation = Some(ScopeConfig {
                id: hostname,
                language: ws_json
                    .get("language")
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
        }

        let userhandle = RuntimeEnv::var("USER")
            .or_else(|_| RuntimeEnv::var("USERNAME"))
            .unwrap_or_else(|_| "unknown".to_string());

        let user_json = Self::load_collection_doc("users", |v| {
            v.get("handle").and_then(|u| u.as_str()) == Some(userhandle.as_str())
        })
        .or_else(|| {
            // Fallback admin uniquement si l'utilisateur OS n'existe pas dans la DB
            Self::load_collection_doc("users", |v| {
                v.get("handle").and_then(|u| u.as_str()) == Some("admin")
            })
        });

        if let Some(doc) = user_json {
            config.user = Some(ScopeConfig {
                id: doc
                    .get("handle")
                    .and_then(|v| v.as_str())
                    .unwrap_or("admin")
                    .to_string(),
                language: doc
                    .get("preferences")
                    .and_then(|p| p.get("language"))
                    .and_then(|v| v.as_str())
                    .map(String::from),
            });
            // On met à jour l'ID de workstation si nécessaire
            config.workstation_id = doc
                .get("default_workstation_id")
                .and_then(|v| v.as_str())
                .unwrap_or("ref:workstations:handle:condorcet")
                .to_string();
        }

        Ok(config)
    }

    fn load_collection_doc<F>(collection_name: &str, predicate: F) -> Option<JsonValue>
    where
        F: Fn(&JsonValue) -> bool,
    {
        let base_domain = dirs::home_dir()?.join("raise_domain");
        // On amorce toujours le boot sur la configuration mère _system
        let db_root = base_domain.join(BOOTSTRAP_DOMAIN).join(BOOTSTRAP_DB);
        let sys_index_path = db_root.join("_system.json");
        let collection_dir = db_root.join("collections").join(collection_name);

        let sys_content = fs::read_to_string_sync(&sys_index_path).ok()?;
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

    fn detect_best_device(config: &AppConfig) -> candle_core::Device {
        // 1. Respect de la frugalité : priorité au CPU si demandé
        if !config.world_model.use_gpu {
            return candle_core::Device::Cpu;
        }

        // 2. Accélération CUDA (Linux/Windows)
        #[cfg(feature = "cuda")]
        {
            // On tente l'index 0 (ta RTX 5060 physique validée par nvidia-smi)
            if let Ok(dev) = candle_core::Device::new_cuda(0) {
                return dev;
            }
        }

        // 3. Accélération Metal (Mac)
        #[cfg(feature = "metal")]
        {
            if let Ok(dev) = candle_core::Device::new_metal(0) {
                return dev;
            }
        }

        // 4. Fallback universel vers le CPU si aucune accélération n'est disponible
        candle_core::Device::Cpu
    }

    pub fn device() -> &'static candle_core::Device {
        DEVICE.get().expect("Device non initialisé")
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
        }
    }
}

impl Default for SimulationContextConfig {
    fn default() -> Self {
        Self {
            source_domain: "mbse2".to_string(),
            source_db: "raise".to_string(),
            target_domain: "sim_mbse2".to_string(),
            target_db: "sim_raise".to_string(),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::core::async_test;

    #[test]
    fn test_scope_config_structure() {
        let scope = ScopeConfig {
            id: "dev-machine".to_string(),
            language: Some("fr".to_string()),
        };
        assert_eq!(scope.id, "dev-machine");
        assert_eq!(scope.language.as_deref(), Some("fr"));
    }

    #[test]
    fn test_deserialize_app_config_with_mount_points() {
        // 🎯 On crée un JSON qui respecte strictement la structure V2
        let json_data = json_value!({
            "active_dapp_id": "ref:dapps:handle:raise_core",
            "workstation_id": "ref:workstations:handle:condorcet",
            "mount_points": {
                "system": { "domain": "_sys_domain", "db": "_sys_db" },
                "raise": { "domain": "_sys_domain", "db": "_raise_core" },
                "exploration": { "domain": "proj1", "db": "sandbox" },
                "modeling": { "domain": "proj1", "db": "mbse" }, // 🎯 On utilise modeling, pas workspace
                "simulation": { "domain": "proj1", "db": "sim" },
                "integration": { "domain": "proj1", "db": "test" },
                "production": { "domain": "proj1", "db": "prod" },
                "operation": { "domain": "proj1", "db": "ops" }
            },
            "core": {
                "env_mode": "test",
                "graph_mode": "none",
                "log_level": "debug",
                "vector_store_provider": "memory",
                "language": "en"
            },
            "paths": { "PATH_TEST": "/tmp" }
        });

        let config: AppConfig =
            json::deserialize_from_value(json_data).expect("Désérialisation échouée");

        // ✅ Vérification des Mount Points
        assert_eq!(config.mount_points.system.domain, "_sys_domain");
        assert_eq!(config.mount_points.modeling.db, "mbse");

        // ✅ Vérification des Identifiants (Strings)
        assert_eq!(config.active_dapp_id, "ref:dapps:handle:raise_core");
        assert_eq!(config.workstation_id, "ref:workstations:handle:condorcet");

        // 💡 Note : config.workstation sera None ici car il est marqué #[serde(skip)].
        // C'est normal, il est peuplé plus tard par load_production_config().
        assert!(config.workstation.is_none());
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_get_service_settings_resolves_correctly() -> RaiseResult<()> {
        use crate::utils::testing::mock::DbSandbox;

        // 1. Initialisation de la Sandbox sécurisée
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        DbSandbox::mock_db(&manager).await?;

        // 2. Création de la collection requise
        manager
            .create_collection(
                "service_configs",
                "db://_system/bootstrap/schemas/v2/dapps/services/service_config.schema.json",
            )
            .await?;

        // 3. Injection d'une fausse configuration Gemini
        let mock_service_id = "ref:services:blueprint:google_gemini";
        let mock_doc = json_value!({
            "_id": "cfg_test_gemini",
            "service_id": mock_service_id,
            "environment": "test",
            "service_settings": {
                "api_key": "TEST_KEY_123",
                "model": "gemini-test-model"
            }
        });

        manager.insert_raw("service_configs", &mock_doc).await?;

        // 4. Test de la fonction métier Zéro Dette
        let settings = AppConfig::get_service_settings(&manager, mock_service_id).await?;

        assert_eq!(settings["api_key"], "TEST_KEY_123");
        assert_eq!(settings["model"], "gemini-test-model");

        Ok(())
    }
}
