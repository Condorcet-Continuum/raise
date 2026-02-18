use crate::utils::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Singleton global pour la configuration
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub name: Option<HashMap<String, String>>,
    pub default_domain: String,
    pub default_db: String,
    pub core: CoreConfig,
    pub paths: HashMap<String, String>,
    pub services: HashMap<String, ServiceConfig>,
    pub ai_engines: HashMap<String, AiEngineConfig>,
    pub integrations: IntegrationsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    pub env_mode: String,
    pub graph_mode: String,
    pub log_level: String,
    pub vector_store_provider: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub status: String,
    pub kind: String,
    pub host: String,
    pub port: u16,
    pub protocol: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AiEngineConfig {
    pub status: String,
    pub provider: String,
    pub model_name: String,
    pub api_url: Option<String>,
    pub api_key: Option<String>,
    pub rust_repo_id: Option<String>,
    pub rust_model_file: Option<String>,
}

// 🔒 SÉCURITÉ : Masquage des clés API dans les logs
impl std::fmt::Debug for AiEngineConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mask = |key: &Option<String>| {
            if key.as_ref().is_some_and(|k| !k.is_empty()) {
                "*** MASQUÉ ***"
            } else {
                "Non configuré"
            }
        };

        f.debug_struct("AiEngineConfig")
            .field("status", &self.status)
            .field("provider", &self.provider)
            .field("model_name", &self.model_name)
            .field("api_url", &self.api_url)
            .field("api_key", &mask(&self.api_key))
            .field("rust_repo_id", &self.rust_repo_id)
            .field("rust_model_file", &self.rust_model_file)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct IntegrationsConfig {
    pub github_token: Option<String>,
    pub compose_profiles: Option<String>,
}

impl std::fmt::Debug for IntegrationsConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mask = |key: &Option<String>| {
            if key.as_ref().is_some_and(|k| !k.is_empty()) {
                "*** MASQUÉ ***"
            } else {
                "Non configuré"
            }
        };
        f.debug_struct("IntegrationsConfig")
            .field("github_token", &mask(&self.github_token))
            .field("compose_profiles", &self.compose_profiles)
            .finish()
    }
}

impl AppConfig {
    pub fn init() -> Result<()> {
        // ✅ SÉCURITÉ : Si la configuration (ou le mock) est déjà chargée, on court-circuite tout !
        if CONFIG.get().is_some() {
            return Ok(());
        }
        let target_env = if cfg!(test) {
            "test".to_string()
        } else if let Ok(env_override) = env::var("RAISE_ENV_MODE") {
            env_override
        } else if cfg!(debug_assertions) {
            "development".to_string()
        } else {
            "production".to_string()
        };

        if target_env == "test" {
            let config = Self::load_test_sandbox()?;
            // On ignore si c'est déjà rempli, car un autre test a pu le faire juste avant
            let _ = CONFIG.set(config);
            return Ok(());
        }

        // --- MODE NORMAL (Cascade JSON-DB via l'Index _system.json) ---

        // ÉTAPE 1 : Configuration Système
        let mut merged_json = Self::load_collection_doc("configs", |v| {
            v.get("core")
                .and_then(|c| c.get("env_mode"))
                .and_then(|e| e.as_str())
                == Some(target_env.as_str())
        })
        .ok_or_else(|| {
            AppError::Config(format!("Config système introuvable pour : {}", target_env))
        })?;

        // ÉTAPE 2 : Surcharge Poste de Travail (BOOTSTRAP: legit std::env)
        let hostname = env::var("HOSTNAME")
            .or_else(|_| env::var("COMPUTERNAME"))
            .unwrap_or_else(|_| "condorcet-ws-01".to_string());

        if let Some(ws_json) = Self::load_collection_doc("workstations", |v| {
            v.get("hostname").and_then(|h| h.as_str()) == Some(hostname.as_str())
        }) {
            tracing::info!(
                "💻 Surcharge : Profil Poste de travail ({}) appliqué.",
                hostname
            );
            json_merge(&mut merged_json, ws_json);
        }

        // ÉTAPE 3 : Surcharge Utilisateur (BOOTSTRAP: legit std::env)
        let username = env::var("USER")
            .or_else(|_| env::var("USERNAME"))
            .unwrap_or_else(|_| "zair".to_string());

        if let Some(user_json) = Self::load_collection_doc("users", |v| {
            v.get("username").and_then(|u| u.as_str()) == Some(username.as_str())
        }) {
            tracing::info!("👤 Surcharge : Profil Utilisateur ({}) appliqué.", username);
            json_merge(&mut merged_json, user_json);
        }

        // ÉTAPE 4 : Validation et Enregistrement avec DEBUG JSON
        let config: AppConfig = match serde_json::from_value(merged_json.clone()) {
            Ok(c) => c,
            Err(e) => return Err(AppError::Config(format!("Erreur JSON : {}", e))),
        };

        if CONFIG.set(config).is_err() && !cfg!(test) {
            tracing::warn!("⚠️ Tentative de ré-initialisation de la config détectée.");
        }

        tracing::info!(
            "⚙️  Config Boot ({}) chargée avec succès (Langue finale: {}).",
            AppConfig::get().core.env_mode,
            AppConfig::get().core.language
        );
        Ok(())
    }

    pub fn get() -> &'static AppConfig {
        CONFIG
            .get()
            .expect("AppConfig non initialisé ! Appelez init().")
    }

    // ✅ CORRECTION : Adapté pour la HashMap
    pub fn get_path(&self, id: &str) -> Option<PathBuf> {
        let path_str = self.paths.get(id)?;
        let base_path = PathBuf::from(path_str);

        // ✅ SOLUTION RÉALISTE POUR GITHUB & PARALLÉLISME
        // En mode test, on simule l'isolation d'instance pour chaque thread.
        // Cela permet à SurrealDB d'avoir son propre fichier "Manifest" sans collision.
        if cfg!(test) && id == "PATH_RAISE_DOMAIN" {
            // On récupère l'ID du thread (l'équivalent d'un ID de conteneur en prod)
            let thread_id = format!("{:?}", std::thread::current().id())
                .replace("ThreadId(", "")
                .replace(")", "");

            let unique_path = base_path.join(format!("instance_{}", thread_id));

            // On s'assure que le dossier existe pour SurrealDB
            let _ = std::fs::create_dir_all(&unique_path);
            return Some(unique_path);
        }

        Some(base_path)
    }

    fn load_collection_doc<F>(collection_name: &str, predicate: F) -> Option<Value>
    where
        F: Fn(&Value) -> bool,
    {
        // ⚓ ANCRE DU DOMAINE : On garde hardcodé le fallback au home_dir pour pouvoir booter
        let base_domain = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("raise_domain");

        let db_root = base_domain.join("_system").join("_system");
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
                                if let Ok(doc_content) = fs::read_to_string(&path) {
                                    if let Ok(doc) = serde_json::from_str::<Value>(&doc_content) {
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

    fn load_test_sandbox() -> Result<Self> {
        let test_config_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("config.test.json");

        let content = fs::read_to_string(&test_config_path)
            .map_err(|e| AppError::Config(format!("Fichier manquant: {:?}", e)))?;

        let mut json_data: Value = serde_json::from_str(&content)
            .map_err(|e| AppError::Config(format!("Erreur syntaxe JSON: {}", e)))?;

        // 1. Transformer le tableau "paths" en Map pour satisfaire la structure Rust
        if let Some(paths_array) = json_data.get("paths").and_then(|p| p.as_array()) {
            let mut paths_map = serde_json::Map::new();
            for item in paths_array {
                if let (Some(id), Some(val)) = (
                    item.get("id").and_then(|i| i.as_str()),
                    item.get("value").and_then(|v| v.as_str()),
                ) {
                    paths_map.insert(id.to_string(), Value::String(val.to_string()));
                }
            }
            json_data["paths"] = Value::Object(paths_map);
        }

        // 🚨 C'EST CETTE LIGNE QUI MANQUAIT : Création de la variable 'config'
        let mut config: AppConfig = serde_json::from_value(json_data)
            .map_err(|e| AppError::Config(format!("Erreur mapping AppConfig: {}", e)))?;

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros();

        let temp_domain =
            env::temp_dir().join(format!("raise_test_{}_{}", std::process::id(), now));
        let _ = fs::create_dir_all(&temp_domain);

        // Maintenant 'config' existe, on peut l'utiliser !
        config.paths.insert(
            "PATH_RAISE_DOMAIN".to_string(),
            temp_domain.to_string_lossy().to_string(),
        );

        Ok(config)
    }
}

fn json_merge(a: &mut Value, b: Value) {
    match (a, b) {
        (Value::Object(a), Value::Object(b)) => {
            for (k, v) in b {
                json_merge(a.entry(k).or_insert(Value::Null), v);
            }
        }
        (a, b) => *a = b,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_recursive_json_merge() {
        let mut system = json!({
            "default_domain": "_system",
            "core": {
                "env_mode": "development",
                "language": "fr"
            }
        });

        let user = json!({
            "core": {
                "language": "en"
            }
        });

        json_merge(&mut system, user);

        assert_eq!(system["core"]["language"], "en");
        assert_eq!(system["default_domain"], "_system");
        assert_eq!(system["core"]["env_mode"], "development");
    }
}

#[cfg(test)]
pub mod test_mocks {
    use super::*;
    use std::collections::HashMap;

    pub fn inject_mock_config() {
        // ✅ Utilise CONFIG (Majuscules) pour le static OnceLock
        if CONFIG.get().is_some() {
            return;
        }

        let mut paths = HashMap::new();
        paths.insert("PATH_LOGS".to_string(), "./temp_logs".to_string());
        paths.insert("PATH_MODELS".to_string(), "./temp_models".to_string());
        paths.insert("PATH_RAISE_DOMAIN".to_string(), "./temp_domain".to_string());

        let mock_config = AppConfig {
            name: None,
            default_domain: "_system".to_string(),
            default_db: "_system".to_string(),
            core: CoreConfig {
                env_mode: "test".to_string(),
                graph_mode: "none".to_string(),
                log_level: "info".to_string(),
                vector_store_provider: "surreal".to_string(),
                language: "fr".to_string(),
            },
            paths,
            services: HashMap::new(),
            ai_engines: HashMap::new(),
            integrations: IntegrationsConfig {
                github_token: None,
                compose_profiles: None,
            },
        };

        // ✅ Utilise CONFIG (Majuscules) ici aussi
        let _ = CONFIG.set(mock_config);
    }
}
