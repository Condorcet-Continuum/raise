use clap::{Parser, Subcommand};

// On garde le module local des commandes
mod commands;

use raise::{
    build_error, user_error, user_info,
    utils::{context, prelude::*},
};

#[derive(Parser)]
#[command(name = "raise-cli")]
#[command(about = "CLI unifié pour la manipulation des modules Raise", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    // Optionnel pour permettre le mode Shell Interactif
    command: Option<Commands>,
}

#[derive(Subcommand, Clone, Debug)]
enum Commands {
    Workflow(commands::workflow::WorkflowArgs),
    ModelEngine(commands::model_engine::ModelArgs),
    Jsondb(commands::jsondb::JsondbArgs),
    Ai(commands::ai::AiArgs),
    Genetics(commands::genetics::GeneticsArgs),
    Blockchain(commands::blockchain::BlockchainArgs),
    Plugins(commands::plugins::PluginsArgs),
    Traceability(commands::traceability::TraceabilityArgs),
    Spatial(commands::spatial::SpatialArgs),
    CodeGen(commands::code_gen::CodeGenArgs),
    Validator(commands::validator::ValidatorArgs),
    Utils(commands::utils::UtilsArgs),
}

#[tokio::main]
async fn main() -> RaiseResult<()> {
    // 1. Initialisation de la Configuration (CRITIQUE)
    if let Err(e) = AppConfig::init() {
        eprintln!("❌ CRITICAL ERROR: Impossible d'initialiser la configuration.");
        eprintln!("   Détails : {}", e);
        std::process::exit(1);
    }

    // 2. Initialisation du Logger
    context::init_logging();

    // 3. Initialisation de la Langue (Lecture directe depuis JSON-DB)
    let config = AppConfig::get();
    context::init_i18n(&config.core.language).await?;

    // Message d'accueil système traduit
    user_info!(
        "CLI_START_INITIALIZED",
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "mode": if cfg!(debug_assertions) { "debug" } else { "release" },
            "component": "cli_engine"
        })
    );

    // 4. Parsing & Dispatch
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            // Mode "One-Shot"
            if let Err(e) = execute_command(cmd.clone()).await {
                // 🛠️ build_error! crée l'objet AppError et logue via tracing,
                // mais il ne contient PAS de 'return', ce qui nous permet de continuer.
                let err = build_error!(
                    "CLI_COMMAND_EXECUTION_FAILED",
                    error = e,
                    context = json!({
                        // On utilise format! car Commands n'implémente pas Serialize
                        "command": format!("{:?}", cmd),
                        "exit_code": 1,
                        "trace": "critical_failure"
                    })
                );

                // 🚩 On affiche l'erreur structurée (avec sa clé, son message et son contexte)
                // puis on coupe proprement le processus.
                eprintln!("{}", err);
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
async fn run_global_shell() -> RaiseResult<()> {
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    println!("--------------------------------------------------");
    println!("🚀 RAISE GLOBAL SHELL - v{}", env!("CARGO_PKG_VERSION"));
    println!("   Tapez 'help' pour la liste des commandes.");
    println!("   Tapez 'exit' ou 'quit' pour quitter.");
    println!("--------------------------------------------------");

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            raise_error!(
                "CLI_EDITOR_INIT_FAILED",
                error = e,
                context = json!({
                    "component": "Rustyline",
                    "terminal_check": "failed"
                })
            );
        }
    };
    let config = AppConfig::get();
    let history_path = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: Le chemin PATH_RAISE_DOMAIN est introuvable !")
        .join("_system")
        .join("history.txt");

    if rl.load_history(&history_path).is_err() {
        // Pas d'historique au premier lancement, c'est normal
    }

    loop {
        let readline = rl.readline("RAISE> ");

        match readline {
            Ok(line) => {
                let input = line.trim();

                if !input.is_empty() {
                    let _ = rl.add_history_entry(input);
                } else {
                    continue;
                }

                if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
                    println!("👋 Au revoir !");
                    break;
                }
                if input.eq_ignore_ascii_case("clear") {
                    print!("\x1B[2J\x1B[1;1H");
                    continue;
                }

                match shell_words::split(input) {
                    Ok(args) => {
                        let mut full_args = vec!["repl".to_string()];
                        full_args.extend(args);

                        match Cli::try_parse_from(full_args) {
                            Ok(cli) => {
                                if let Some(cmd) = cli.command {
                                    // 1. On clone pour garder la propriété de 'cmd' en cas d'erreur
                                    if let Err(e) = execute_command(cmd.clone()).await {
                                        // 🚀 Utilisation du Cas 2 : Métadonnées structurées
                                        user_error!(
                                            "CLI_COMMAND_EXECUTION_FAILED",
                                            json!({
                                                // 2. On utilise le format Debug pour contourner l'absence de Serialize
                                                "command": format!("{:?}", cmd),
                                                "error_detail": format!("{:?}", e),
                                                "context": "interactive_repl_execution"
                                            })
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                e.print().ok();
                            }
                        }
                    }
                    Err(e) => eprintln!("❌ Erreur de syntaxe : {}", e),
                }
            }
            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => break,
            Err(err) => {
                // 🚀 Capture structurée avant la rupture de la boucle
                user_error!(
                    "CLI_SHELL_FATAL_ERROR",
                    json!({
                        "error": format!("{:?}", err),
                        "termination": "loop_break",
                        "context": "interactive_shell_session"
                    })
                );
                break;
            }
        }
    }

    if let Err(e) = rl.save_history(&history_path) {
        tracing::warn!("Impossible de sauvegarder l'historique : {}", e);
    }

    Ok(())
}

async fn execute_command(cmd: Commands) -> RaiseResult<()> {
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
