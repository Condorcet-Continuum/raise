use clap::{Parser, Subcommand};

// On garde le module local des commandes
mod commands;

use raise::{
    user_error,
    user_info, // Les macros à la racine
    utils::{
        context,    // Remplace env, i18n, logger (Config & Environnement)
        data,       // Remplace json (Manipulation de données)
        io,         // Remplace fs (Entrées/Sorties sécurisées)
        prelude::*, // Types communs (Result, AppError, etc.)
    },
};

// EMBARQUEMENT DES RESSOURCES (Compilation)
const DEFAULT_LOCALE_FR: &str = include_str!("../../../locales/fr.json");
const DEFAULT_LOCALE_EN: &str = include_str!("../../../locales/en.json");
const DEFAULT_LOCALE_DE: &str = include_str!("../../../locales/de.json");
const DEFAULT_LOCALE_ES: &str = include_str!("../../../locales/es.json");

#[derive(Parser)]
#[command(name = "raise-cli")]
#[command(about = "CLI unifié pour la manipulation des modules Raise", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    // Optionnel pour permettre le mode Shell Interactif
    command: Option<Commands>,
}

#[derive(Subcommand, Clone)]
enum Commands {
    /// Pilotage du Workflow Engine (Neuro-Symbolic MAS)
    Workflow(commands::workflow::WorkflowArgs),

    ModelEngine(commands::model_engine::ModelArgs),

    /// Commandes pour le JSON-DB (Embedded NoSQL Engine)
    Jsondb(commands::jsondb::JsondbArgs),

    /// Commandes pour le AI (Cerveau Neuro-Symbolique)
    Ai(commands::ai::AiArgs),

    Genetics(commands::genetics::GeneticsArgs),

    Blockchain(commands::blockchain::BlockchainArgs),

    Plugins(commands::plugins::PluginsArgs),

    Traceability(commands::traceability::TraceabilityArgs),

    Spatial(commands::spatial::SpatialArgs),

    CodeGen(commands::code_gen::CodeGenArgs),

    /// Commandes pour le Validator (Schémas & Validation)
    Validator(commands::validator::ValidatorArgs),

    Utils(commands::utils::UtilsArgs),
}

#[tokio::main]
async fn main() -> Result<()> {
    // 1. Initialisation de la Configuration (CRITIQUE)
    if let Err(e) = AppConfig::init() {
        eprintln!("❌ CRITICAL ERROR: Impossible d'initialiser la configuration.");
        eprintln!("   Détails : {}", e);
        std::process::exit(1);
    }

    // 2. Initialisation du Logger
    context::init_logging();

    // 3. BOOTSTRAP DES LOCALES
    // Async et Atomique !
    bootstrap_locales().await;

    // 4. Initialisation de la Langue
    // REFAC: Utilisation de env::get_or
    let lang = context::get_or("RAISE_LANG", "fr");
    context::init_i18n(&lang);

    // Message d'accueil système
    user_info!("CLI_START", "v{}", env!("CARGO_PKG_VERSION"));

    // 5. Parsing & Dispatch
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            // Mode "One-Shot"
            if let Err(e) = execute_command(cmd).await {
                user_error!("CMD_FAIL", "{}", e);
                std::process::exit(1);
            }
        }
        None => {
            // Mode "Global Shell"
            run_global_shell().await?;
        }
    }

    tracing::debug!("Fin de l'exécution du CLI");
    Ok(())
}

/// Boucle principale du Shell Global (REPL)
async fn run_global_shell() -> Result<()> {
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    println!("--------------------------------------------------");
    println!("🚀 RAISE GLOBAL SHELL - v{}", env!("CARGO_PKG_VERSION"));
    println!("   Tapez 'help' pour la liste des commandes.");
    println!("   Tapez 'exit' ou 'quit' pour quitter.");
    println!("--------------------------------------------------");

    // 1. Initialisation de l'éditeur de ligne
    let mut rl = DefaultEditor::new().map_err(|e| e.to_string())?;
    // 2. Chargement de l'historique existant (si disponible)
    let config = AppConfig::get();
    let history_path = config.database_root.join("history.txt");

    if rl.load_history(&history_path).is_err() {
        // Pas d'historique ou fichier introuvable (normal au premier lancement)
    }

    loop {
        // Affiche le prompt et attend l'input (avec gestion des flèches !)
        let readline = rl.readline("RAISE> ");

        match readline {
            Ok(line) => {
                let input = line.trim();

                // Si la ligne n'est pas vide, on l'ajoute à l'historique mémoire
                if !input.is_empty() {
                    let _ = rl.add_history_entry(input);
                } else {
                    continue;
                }

                // Commandes natives du Shell
                if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                    println!("👋 Au revoir !");
                    break;
                }
                if input.eq_ignore_ascii_case("clear") {
                    print!("\x1B[2J\x1B[1;1H");
                    continue;
                }

                // Parsing et Exécution
                match shell_words::split(input) {
                    Ok(args) => {
                        let mut full_args = vec!["repl".to_string()];
                        full_args.extend(args);

                        match Cli::try_parse_from(full_args) {
                            Ok(cli) => {
                                if let Some(cmd) = cli.command {
                                    if let Err(e) = execute_command(cmd).await {
                                        user_error!("CMD_FAIL", "{}", e);
                                    }
                                }
                            }
                            Err(e) => {
                                e.print().ok();
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("❌ Erreur de syntaxe : {}", e);
                    }
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("(CTRL-C)");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("(CTRL-D)");
                break;
            }
            Err(err) => {
                user_error!("SHELL_ERROR", "{}", err);
                break;
            }
        }
    }

    // 3. Sauvegarde de l'historique en quittant
    if let Err(e) = rl.save_history(&history_path) {
        tracing::warn!("Impossible de sauvegarder l'historique : {}", e);
    }

    Ok(())
}

async fn execute_command(cmd: Commands) -> Result<()> {
    match cmd {
        Commands::Workflow(args) => commands::workflow::handle(args).await,
        Commands::ModelEngine(args) => commands::model_engine::handle(args).await,
        Commands::Jsondb(args) => commands::jsondb::handle(args).await,
        Commands::Ai(args) => commands::ai::handle(args).await,
        Commands::Genetics(args) => commands::genetics::handle(args).await,
        Commands::Blockchain(args) => commands::blockchain::handle(args).await,
        Commands::Plugins(args) => commands::plugins::handle(args).await,
        Commands::Traceability(args) => commands::traceability::handle(args).await,
        Commands::Spatial(args) => commands::spatial::handle(args).await,
        Commands::CodeGen(args) => commands::code_gen::handle(args).await,
        Commands::Validator(args) => commands::validator::handle(args).await,
        Commands::Utils(args) => commands::utils::handle(args).await,
    }
}

/// Déploie les fichiers de langue de manière Atomique et Validée
async fn bootstrap_locales() {
    let config = AppConfig::get();
    let locales_dir = config.database_root.join("locales");

    // REFAC: Utilisation de fs::ensure_dir (plus sûr)
    if let Err(e) = io::ensure_dir(&locales_dir).await {
        tracing::warn!("Impossible de créer le dossier locales : {}", e);
        return;
    }

    let writes = vec![
        ("fr.json", DEFAULT_LOCALE_FR),
        ("en.json", DEFAULT_LOCALE_EN),
        ("de.json", DEFAULT_LOCALE_DE),
        ("es.json", DEFAULT_LOCALE_ES),
    ];

    for (name, content) in writes {
        let path = locales_dir.join(name);

        // 1. Validation : On parse le JSON brut
        match data::parse::<data::Value>(content) {
            Ok(json_value) => {
                // 2. Écriture Atomique : On utilise notre utilitaire sécurisé
                if let Err(e) = io::write_json_atomic(&path, &json_value).await {
                    tracing::error!("Erreur écriture atomique {}: {}", name, e);
                } else {
                    tracing::debug!("Locale {} déployée.", name);
                }
            }
            Err(e) => {
                tracing::error!("❌ locale {} corrompue au build ! : {}", name, e);
            }
        }
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn verify_cli_structure() {
        Cli::command().debug_assert();
    }

    #[test]
    fn test_help_generation() {
        let output = Cli::command().render_help().to_string();
        assert!(output.contains("raise-cli"));
        assert!(output.contains("jsondb"));
    }

    #[test]
    fn test_dispatch_ai() {
        let args = vec!["raise-cli", "ai"];
        let cli = Cli::try_parse_from(args).expect("Parsing failed");
        match cli.command {
            Some(Commands::Ai(_)) => assert!(true),
            _ => panic!("Le dispatch vers le module AI a échoué"),
        }
    }

    #[test]
    fn test_shell_words_parsing() {
        let input = "ai classify \"hello world\"";
        let args = shell_words::split(input).unwrap();
        assert_eq!(args.len(), 3);
        assert_eq!(args[2], "hello world");
    }
}
