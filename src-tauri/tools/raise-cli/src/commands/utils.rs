// FICHIER : src-tauri/tools/raise-cli/src/commands/utils.rs

use clap::{Args, Subcommand};
use raise::{
    user_info, user_success,
    utils::{
        config::AppConfig, // Nécessaire pour AppConfig::get()
        io::{self},
        prelude::*,
    },
};

// 🎯 NOUVEAU : Imports pour vérifier la BDD
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::storage::{JsonDbConfig, StorageEngine};

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
            let config = AppConfig::get();

            println!("--- 🛠️ RAISE SYSTEM INFO ---");
            user_info!("VERSION", "{}", env!("CARGO_PKG_VERSION"));

            let env_mode = if cfg!(debug_assertions) {
                "development"
            } else {
                "production"
            };
            user_info!("SYS_ENV", "Environnement : {}", env_mode);

            let db_root = config.get_path("PATH_RAISE_DOMAIN");
            user_info!("DB_ROOT", "{:?}", db_root);

            let mut provider = String::from("Non configuré");
            let mut model = String::from("Inconnu");
            let mut status = String::from("disabled");

            // 🎯 Vérification du composant LLM directement en base de données
            if let Some(ref path) = db_root {
                let storage = StorageEngine::new(JsonDbConfig::new(path.clone()));
                let manager =
                    CollectionsManager::new(&storage, &config.system_domain, &config.system_db);

                if let Ok(settings) = AppConfig::get_component_settings(&manager, "llm").await {
                    provider = settings
                        .get("provider")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Local")
                        .to_string();
                    model = settings
                        .get("rust_model_file")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Inconnu")
                        .to_string();
                    status = "enabled".to_string();
                }
            }

            user_info!(
                "LLM_ENGINE",
                "Provider: {} | Modèle: {} | Statut: {}",
                provider,
                model,
                status
            );

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
        let args = UtilsArgs {
            command: UtilsCommands::Ping,
        };
        assert!(handle(args).await.is_ok());
    }

    #[tokio::test]
    async fn test_utils_info() {
        raise::utils::config::test_mocks::inject_mock_config();

        let args = UtilsArgs {
            command: UtilsCommands::Info,
        };

        assert!(handle(args).await.is_ok());
    }
}
