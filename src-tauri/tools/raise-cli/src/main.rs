use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;

// On importe le dossier commands/ où sont rangés nos modules
mod commands;

#[derive(Parser)]
#[command(name = "raise-cli")]
#[command(about = "CLI unifié pour la manipulation des modules Raise", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Commandes pour le JSON-DB (Embedded NoSQL Engine)
    Jsondb(commands::jsondb::JsondbArgs),
    // C'est ici que nous ajouterons les futurs modules :
    // Ai(commands::ai::AiArgs),
    // Blockchain(...),
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Chargement de l'environnement (.env)
    dotenv().ok();

    // 2. Initialisation des logs si RUST_LOG est défini
    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt::init();
    }

    // 3. Parsing des arguments
    let cli = Cli::parse();

    // 4. Dispatch vers les modules
    match cli.command {
        Commands::Jsondb(args) => {
            // On passe la main au handler du module dédié
            commands::jsondb::handle(args).await?;
        }
    }

    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli_structure() {
        // Vérifie que la configuration Clap est valide (noms uniques, types corrects, etc.)
        Cli::command().debug_assert();
    }

    #[test]
    fn test_help_generation() {
        // Vérifie que l'aide peut être générée sans paniquer
        let output = Cli::command().render_help();
        assert!(output.to_string().contains("RAISE JSON-DB"));
    }

    #[test]
    fn test_dispatch_jsondb() {
        // Simule une commande "raise-cli jsondb list-collections"
        let args = vec!["raise-cli", "jsondb", "list-collections"];
        let cli = Cli::try_parse_from(args).expect("Parsing failed");

        match cli.command {
            Commands::Jsondb(jsondb_args) => {
                // On vérifie qu'on est bien tombé dans le bon variant de l'enum
                match jsondb_args.command {
                    commands::jsondb::JsondbCommands::ListCollections => assert!(true),
                    _ => panic!("Mauvaise sous-commande parsée"),
                }
            } // _ => panic!("Mauvais module parsé"), // Commenté car seul Jsondb existe pour l'instant
        }
    }

    #[test]
    fn test_global_version() {
        // Vérifie que la version est bien récupérée du Cargo.toml
        let version = Cli::command().get_version().unwrap();
        assert!(!version.is_empty());
    }
}
