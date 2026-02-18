// FICHIER : src-tauri/src/bin/raise-cli/utils.rs

use clap::{Args, Subcommand};
use raise::{
    user_info, user_success,
    utils::{
        config::AppConfig, // Nécessaire pour AppConfig::get()
        io::{self},
        prelude::*,
    },
};

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

pub async fn handle(args: UtilsArgs) -> Result<()> {
    match args.command {
        UtilsCommands::Info => {
            // Singleton Config (Doit être initialisé avant)
            let config = AppConfig::get();

            println!("--- 🛠️ RAISE SYSTEM INFO ---");
            user_info!("VERSION", "{}", env!("CARGO_PKG_VERSION"));

            // Champs valides confirmés par le compilateur
            let env_mode = if cfg!(debug_assertions) {
                "development"
            } else {
                "production"
            };
            user_info!("SYS_ENV", "Environnement : {}", env_mode);

            // Utilisation robuste de get_path
            let db_root = config.get_path("PATH_RAISE_DOMAIN");
            user_info!("DB_ROOT", "{:?}", db_root);

            // Affichage masqué pour la clé API si elle existe (sécurité)
            let has_key = config
                .ai_engines
                .get("cloud_gemini")
                .and_then(|engine| engine.api_key.as_ref())
                .map(|k| !k.is_empty())
                .unwrap_or(false);

            let api_url = config
                .ai_engines
                .get("primary_local")
                .and_then(|engine| engine.api_url.as_deref())
                .unwrap_or("Non configurée");

            user_info!("LLM_API", "URL: {} (Key set: {})", api_url, has_key);

            // Vérification simple de l'existence de la racine DB
            if let Some(path) = db_root {
                if io::exists(&path).await {
                    user_success!("CHECK_FS", "Le dossier database_root est accessible.");
                } else {
                    eprintln!(
                        "❌ CHECK_FS: Le dossier database_root semble manquant ! ({:?})",
                        path
                    );
                }
            } else {
                eprintln!("❌ CHECK_FS: Configuration PATH_RAISE_DOMAIN manquante !");
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
        // Ping ne dépend pas de la config, donc pas besoin d'init
        let args = UtilsArgs {
            command: UtilsCommands::Ping,
        };
        assert!(handle(args).await.is_ok());
    }

    #[tokio::test]
    async fn test_utils_info() {
        // ✅ CORRECTION : Utilisation du Mock Mémoire
        // Au lieu de chercher un fichier json sur le disque (fragile),
        // on injecte la config directement en mémoire.
        raise::utils::config::test_mocks::inject_mock_config();

        let args = UtilsArgs {
            command: UtilsCommands::Info,
        };

        // Cela ne devrait plus paniquer sur AppConfig::get()
        assert!(handle(args).await.is_ok());
    }
}
