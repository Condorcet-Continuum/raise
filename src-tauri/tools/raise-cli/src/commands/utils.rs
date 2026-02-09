use clap::{Args, Subcommand};
use raise::utils::config::AppConfig;
use raise::utils::error::AnyResult;
use raise::{user_info, user_success};

/// Outils de maintenance et d'inspection système pour RAISE.
#[derive(Args, Clone, Debug)]
pub struct UtilsArgs {
    #[command(subcommand)]
    pub command: UtilsCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum UtilsCommands {
    /// Affiche la configuration active et les chemins critiques
    Info,
    /// Vérifie la connectivité interne (Ping)
    Ping,
}

pub async fn handle(args: UtilsArgs) -> AnyResult<()> {
    match args.command {
        UtilsCommands::Info => {
            // Singleton Config
            let config = AppConfig::get();

            println!("--- 🛠️ RAISE SYSTEM INFO ---");
            user_info!("VERSION", "{}", env!("CARGO_PKG_VERSION"));

            // Champs valides confirmés par le compilateur
            user_info!("ENV_MODE", "{:?}", config.env_mode);
            user_info!("DB_ROOT", "{:?}", config.database_root);

            // Affichage masqué pour la clé API si elle existe (sécurité)
            let has_key = config
                .llm_api_key
                .as_ref()
                .map(|k| !k.is_empty())
                .unwrap_or(false);
            user_info!(
                "LLM_API",
                "URL: {} (Key set: {})",
                config.llm_api_url,
                has_key
            );

            // Vérification simple de l'existence de la racine DB
            if raise::utils::fs::exists(&config.database_root).await {
                user_success!("CHECK_FS", "Le dossier database_root est accessible.");
            } else {
                // Note: user_error! n'est pas importé, on utilise un log simple ou on l'ajoute aux imports
                eprintln!("❌ CHECK_FS: Le dossier database_root semble manquant !");
            }
        }

        UtilsCommands::Ping => {
            user_success!("PONG", "Raise-CLI est opérationnel.");
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_utils_ping() {
        let args = UtilsArgs {
            command: UtilsCommands::Ping,
        };
        assert!(handle(args).await.is_ok());
    }

    #[tokio::test]
    async fn test_utils_info() {
        // On tente d'init la config pour le test, on ignore l'erreur si déjà init
        let _ = AppConfig::init();

        let args = UtilsArgs {
            command: UtilsCommands::Info,
        };
        assert!(handle(args).await.is_ok());
    }
}
