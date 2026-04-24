// FICHIER : src-tauri/src/utils/data/config.rs

// 1. Base de données (AI-Ready Queries)
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter};
// 2. Core : Environnement, Concurrence et Erreurs
use crate::utils::core::error::RaiseResult;
use crate::utils::core::{RuntimeEnv, StaticCell};
use crate::{raise_error, user_debug, user_warn};

// 3. I/O : Système de fichiers
use crate::utils::io::fs::PathBuf;

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
    #[serde(rename = "_id")]
    pub id: String,

    #[serde(rename = "_created_at")]
    pub created_at: String,

    #[serde(rename = "_updated_at")]
    pub updated_at: String,

    #[serde(rename = "@type", deserialize_with = "deserialize_type_flexible")]
    pub semantic_type: Vec<String>,

    pub name: Option<UnorderedMap<String, String>>,

    // --- LA COLONNE VERTÉBRALE (VITAL) ---
    pub mount_points: MountPointsConfig,
    pub core: CoreConfig,

    #[serde(deserialize_with = "deserialize_paths_flexible")]
    pub paths: UnorderedMap<String, String>,

    // --- IDENTIFIANTS DE BOOT ---
    pub active_dapp_id: String,
    pub workstation_id: String,

    #[serde(default)]
    pub active_services: Vec<String>,
    #[serde(default)]
    pub active_components: Vec<String>,

    // --- SCOPES RUNTIME (DYNAMIQUE) ---
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
fn deserialize_type_flexible<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: CustomDeserializerEngine<'de>,
{
    let v: JsonValue = Deserializable::deserialize(deserializer)?;

    if let Some(s) = v.as_str() {
        // Si c'est une simple chaîne de caractères, on l'enveloppe dans un tableau
        Ok(vec![s.to_string()])
    } else if let Some(arr) = v.as_array() {
        // Si c'est déjà un tableau, on le lit proprement
        let mut types = Vec::new();
        for item in arr {
            if let Some(s) = item.as_str() {
                types.push(s.to_string());
            }
        }
        Ok(types)
    } else {
        // 🎯 FINI LES FALLBACKS : Si le format est mauvais, on bloque la désérialisation
        Err(DeserializationErrorTrait::custom(
            "Le champ '@type' est invalide. Attendu : String ou Array de Strings.",
        ))
    }
}

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
    pub use_gpu: bool,
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

    pub async fn get_runtime_settings(
        manager: &CollectionsManager<'_>,
        target_ref: &str, // ex: "ref:components:handle:ai_llm" ou "ref:services:handle:svc_ai"
    ) -> RaiseResult<JsonValue> {
        let config = AppConfig::get();

        let id_to_query = match manager.resolve_single_reference(target_ref).await {
            Ok(uuid) => uuid,
            Err(_) => {
                user_debug!("CFG_SEMANTIC_FALLBACK", json_value!({"handle": target_ref}));
                target_ref.to_string()
            }
        };

        // 🛡️ 1. LE GATEKEEPER : Vérification du statut d'activation
        let is_active = config.active_services.contains(&target_ref.to_string())
            || config.active_components.contains(&target_ref.to_string())
            || config.active_services.contains(&id_to_query)
            || config.active_components.contains(&id_to_query);

        if !is_active {
            user_warn!(
                "WRN_MODULE_INACTIVE",
                json_value!({
                    "target": target_ref,
                    "hint": "Ce module a demandé à démarrer, mais il n'est pas déclaré dans les listes 'active_services' ou 'active_components' du système."
                })
            );
        }

        // 🔍 2. RÉSOLUTION SÉMANTIQUE (Smart Link -> UUID)
        let id_to_query = match manager.resolve_single_reference(target_ref).await {
            Ok(uuid) => uuid,
            Err(_) => {
                user_debug!("CFG_SEMANTIC_FALLBACK", json_value!({"handle": target_ref}));
                target_ref.to_string()
            }
        };

        // 🛤️ 3. DÉTERMINATION DU CHAMP DE RECHERCHE
        // Si c'est un composant, on cherche par 'component_id', sinon par 'service_id'
        let join_field = if target_ref.contains("components:") {
            "component_id"
        } else {
            "service_id"
        };

        // 💾 4. REQUÊTE SUR LA BASE DE DONNÉES (service_configs)
        let mut query = Query::new("service_configs");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![Condition::eq(join_field, json_value!(id_to_query.clone()))],
        });
        query.limit = Some(1);

        let result = QueryEngine::new(manager).execute_query(query).await?;

        // 📦 5. EXTRACTION DU DICTIONNAIRE
        if let Some(doc) = result.documents.into_iter().next() {
            if let Some(settings) = doc.get("service_settings") {
                return Ok(settings.clone());
            }
        }

        raise_error!(
            "ERR_CONFIG_MISSING_SETTINGS",
            error = "Le document service_config a été trouvé, mais le dictionnaire 'service_settings' est absent.",
            context = json_value!({ "target": target_ref, "queried_id": id_to_query })
        );
    }

    fn load_production_config(env: &str) -> RaiseResult<Self> {
        // 🎯 1. DÉTERMINISME : On lit le nom exact du profil ciblé (par défaut "raise_core")
        let target_profile =
            RuntimeEnv::var("RAISE_PROFILE").unwrap_or_else(|_| "raise_core".to_string());

        // 🎯 2. RECHERCHE EXACTE : On ne cherche plus par "env_mode", mais par "handle"
        let system_json = Self::load_collection_doc("configs", |v| {
            v.get("handle").and_then(|h| h.as_str()) == Some(target_profile.as_str())
                || v.get("_id").and_then(|id| id.as_str()) == Some(target_profile.as_str())
        });

        let Some(json_val) = system_json else {
            raise_error!(
                "ERR_CONFIG_SYS_MISSING",
                error = format!("Configuration système '{}' introuvable.", target_profile),
                context = json_value!({
                    "target_profile": target_profile,
                    "target_environment": env,
                    "hint": "Vérifiez qu'un fichier avec ce handle existe dans _system/bootstrap/collections/configs/"
                })
            );
        };

        let mut config: AppConfig = match json::deserialize_from_value(json_val.clone()) {
            Ok(c) => c,
            Err(e) => {
                // 🎯 Debug direct en cas d'échec
                eprintln!(
                    "❌ [RAISE FATAL] Erreur critique de désérialisation : {}",
                    e
                );

                raise_error!(
                    "ERR_CONFIG_DESERIALIZE",
                    error = e.to_string(),
                    context = json_value!({
                        "profile": target_profile,
                        "cause_exacte": e.to_string(),
                        "json_brut": json_val
                    })
                )
            }
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

        // 🎯 L'adresse du BIOS devient dynamique (Overrides via Environnement)
        let bios_domain = RuntimeEnv::var("RAISE_BOOTSTRAP_DOMAIN")
            .unwrap_or_else(|_| BOOTSTRAP_DOMAIN.to_string());
        let bios_db =
            RuntimeEnv::var("RAISE_BOOTSTRAP_DB").unwrap_or_else(|_| BOOTSTRAP_DB.to_string());

        let collection_dir = base_domain
            .join(bios_domain)
            .join(bios_db)
            .join("collections")
            .join(collection_name);
        if !collection_dir.exists() {
            return None;
        }

        // 🎯 3. FIN DE L'OEUF ET LA POULE : On scanne directement les fichiers au lieu
        // de dépendre de _system.json (qui n'existe peut-être pas encore si le crash s'est produit pendant sa mise à jour)
        if let Ok(entries) = std::fs::read_dir(collection_dir) {
            for entry in entries.flatten() {
                if entry.path().extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Ok(content) = std::fs::read_to_string(entry.path()) {
                        if let Ok(doc) = json::deserialize_from_str::<JsonValue>(&content) {
                            if predicate(&doc) {
                                return Some(doc); // On retourne le premier match EXACT
                            }
                        }
                    }
                }
            }
        }
        None
    }

    fn detect_best_device(config: &AppConfig) -> candle_core::Device {
        // 1. Respect de la frugalité : priorité au CPU si demandé
        if !config.core.use_gpu {
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
            "_id": "cfg_test",
            "_created_at": "2026-01-01T00:00:00Z",
            "_updated_at": "2026-01-01T00:00:00Z",
            "@type": ["Configuration"],
            "active_dapp_id": "ref:dapps:handle:raise_core",
            "workstation_id": "ref:workstations:handle:condorcet",
            "mount_points": {
                "system": { "domain": "_sys_domain", "db": "_sys_db" },
                "raise": { "domain": "_sys_domain", "db": "_raise_core" },
                "exploration": { "domain": "proj1", "db": "sandbox" },
                "modeling": { "domain": "proj1", "db": "mbse" },
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
                "language": "en",
                "use_gpu": false
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
    async fn test_get_runtime_settings_resolves_correctly() -> RaiseResult<()> {
        use crate::utils::testing::mock::DbSandbox;

        let sandbox = DbSandbox::new().await?;
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );

        DbSandbox::mock_db(&manager).await?;

        let generic_schema = "db://_system/bootstrap/schemas/v1/db/generic.schema.json";
        manager
            .create_collection("services", generic_schema)
            .await?;

        let expected_uuid = "phys-uuid-gemini";
        manager
            .insert_raw(
                "services",
                &json_value!({ "_id": expected_uuid, "blueprint": "google_gemini" }),
            )
            .await?;

        manager
            .create_collection("service_configs", generic_schema)
            .await?;

        let mock_service_id = "ref:services:blueprint:google_gemini";

        let mock_doc = json_value!({
            "_id": "cfg_test_gemini",
            "handle": "cfg_test_gemini",
            "service_id": mock_service_id,
            "environment": "test",
            "service_settings": {
                "api_key": "TEST_KEY_123",
                "model": "gemini-test-model"
            }
        });

        manager.upsert_document("service_configs", mock_doc).await?;

        // 🎯 ON TESTE LE GATEKEEPER (get_runtime_settings)
        let settings = AppConfig::get_runtime_settings(&manager, mock_service_id).await?;

        assert_eq!(settings["api_key"], "TEST_KEY_123");
        assert_eq!(settings["model"], "gemini-test-model");

        Ok(())
    }
}
