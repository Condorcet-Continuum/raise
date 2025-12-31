use crate::utils::error::{AppError, Result};
use std::env;
use std::path::PathBuf;
use std::sync::OnceLock;

/// Singleton global pour la configuration (accessible partout via AppConfig::get())
static CONFIG: OnceLock<AppConfig> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub env_mode: String,
    pub database_root: PathBuf,
    pub llm_api_url: String,
    pub llm_api_key: Option<String>,
}

impl AppConfig {
    /// Charge la configuration depuis l'environnement (.env).
    /// Doit être appelé au démarrage dans le main.rs.
    pub fn init() -> Result<()> {
        // Charge le fichier .env s'il existe
        dotenvy::dotenv().ok();

        let config = AppConfig {
            env_mode: env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),

            // Chemin critique : Stockage des données (~/raise_domain par défaut)
            database_root: env::var("PATH_RAISE_DOMAIN")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    dirs::home_dir()
                        .unwrap_or(PathBuf::from("."))
                        .join("raise_domain")
                }),

            // URL par défaut pour le LLM local (Docker/Ollama)
            llm_api_url: env::var("RAISE_LOCAL_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),

            // Clé API optionnelle (pour le Cloud Gemini/OpenAI)
            llm_api_key: env::var("RAISE_GEMINI_KEY").ok(),
        };

        // On initialise le singleton. Si déjà fait, on renvoie une erreur.
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

    /// Accesseur global sécurisé (panique avec message clair si init() oublié)
    pub fn get() -> &'static AppConfig {
        CONFIG
            .get()
            .expect("AppConfig non initialisé ! Appelez AppConfig::init() au début du main.")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    // Note : Comme AppConfig est un Singleton (OnceLock), on ne peut l'initialiser
    // qu'une seule fois par processus de test. On fait donc un test unique et complet.
    #[test]
    fn test_config_initialization() {
        // 1. Préparer l'environnement simulé
        // On force des valeurs spécifiques pour être sûr que le test soit indépendant du fichier .env réel
        env::set_var("APP_ENV", "test_mode");
        env::set_var("RAISE_LOCAL_URL", "http://127.0.0.1:9090");
        env::set_var("RAISE_GEMINI_KEY", "test_key_forced"); // On force une clé bidon

        // 2. Initialiser
        let init_result = AppConfig::init();

        // Gestion du Singleton : si déjà init par un autre test, ce n'est pas grave
        if init_result.is_ok() {
            // Si c'est nous qui l'avons initialisé, on vérifie que nos vars sont prises en compte
            let config = AppConfig::get();
            assert_eq!(config.env_mode, "test_mode");
            assert_eq!(config.llm_api_url, "http://127.0.0.1:9090");

            // Correction ici : on vérifie que la config a bien lu notre variable forcée
            // au lieu de vérifier qu'elle est vide.
            assert_eq!(config.llm_api_key, Some("test_key_forced".to_string()));

            assert!(config
                .database_root
                .to_string_lossy()
                .contains("raise_domain"));
        } else {
            // Si la config était déjà chargée (par un autre test en parallèle),
            // on ne peut pas garantir les valeurs, donc on ignore ou on vérifie juste que ça existe.
            assert!(CONFIG.get().is_some());
        }
    }
}
