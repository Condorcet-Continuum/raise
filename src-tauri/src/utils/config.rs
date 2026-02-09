use crate::utils::error::{AppError, Result};
use serde::{Deserialize, Serialize};
use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Singleton global pour la configuration
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub env_mode: String,
    pub database_root: PathBuf,
    pub llm_api_url: String,
    pub llm_api_key: Option<String>,
}

impl AppConfig {
    /// Charge la configuration depuis l'environnement réel (.env + Vars système).
    pub fn init() -> Result<()> {
        // 1. Charge le fichier .env
        dotenvy::dotenv().ok();

        // 2. Construit la config en utilisant la vraie fonction std::env::var
        let config = Self::build_from_source(|key| env::var(key).ok());

        // 3. Initialise le singleton
        CONFIG
            .set(config)
            .map_err(|_| AppError::Config("La configuration a déjà été initialisée".to_string()))?;

        tracing::info!(
            "⚙️  Configuration chargée (Env: {}, DB: {:?})",
            AppConfig::get().env_mode,
            AppConfig::get().database_root
        );
        Ok(())
    }

    /// Accesseur global
    pub fn get() -> &'static AppConfig {
        CONFIG
            .get()
            .expect("AppConfig non initialisé ! Appelez AppConfig::init() au début du main.")
    }

    /// Méthode "Pure" (sans effets de bord) qui construit la config.
    fn build_from_source<F>(env_provider: F) -> Self
    where
        F: Fn(&str) -> Option<String>,
    {
        AppConfig {
            // CORRECTION : || (0 argument) au lieu de |_| (1 argument)
            env_mode: env_provider("APP_ENV").unwrap_or_else(|| "development".to_string()),

            database_root: env_provider("PATH_RAISE_DOMAIN")
                .map(PathBuf::from)
                .unwrap_or_else(|| {
                    // CORRECTION : ||
                    // Fallback sur le dossier home
                    dirs::home_dir()
                        .unwrap_or(PathBuf::from("."))
                        .join("raise_domain")
                }),

            llm_api_url: env_provider("RAISE_LOCAL_URL")
                .unwrap_or_else(|| "http://localhost:8080".to_string()), // CORRECTION : ||

            llm_api_key: env_provider("RAISE_GEMINI_KEY"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_config_logic_pure() {
        // 1. Définir un "Mock" de l'environnement via une simple closure
        let mock_env = |key: &str| -> Option<String> {
            match key {
                "APP_ENV" => Some("test_pure_mode".to_string()),
                "RAISE_LOCAL_URL" => Some("http://mock-url:1234".to_string()),
                "RAISE_GEMINI_KEY" => Some("secret_mock_key".to_string()),
                "PATH_RAISE_DOMAIN" => Some("/tmp/mock_domain".to_string()),
                _ => None,
            }
        };

        // 2. Construire la config avec ce mock
        let config = AppConfig::build_from_source(mock_env);

        // 3. Vérifications
        assert_eq!(config.env_mode, "test_pure_mode");
        assert_eq!(config.llm_api_url, "http://mock-url:1234");
        assert_eq!(config.llm_api_key, Some("secret_mock_key".to_string()));
        assert_eq!(config.database_root, PathBuf::from("/tmp/mock_domain"));
    }

    #[test]
    fn test_config_defaults() {
        // Test pour vérifier les valeurs par défaut (quand les variables manquent)
        let empty_env = |_: &str| -> Option<String> { None };

        let config = AppConfig::build_from_source(empty_env);

        assert_eq!(config.env_mode, "development");
        assert_eq!(config.llm_api_url, "http://localhost:8080");
        assert!(config.llm_api_key.is_none());
        assert!(config
            .database_root
            .to_string_lossy()
            .ends_with("raise_domain"));
    }
}
