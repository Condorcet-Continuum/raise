// FICHIER : src-tauri/tools/raise-cli/src/main.rs

use clap::{Parser, Subcommand};

// On garde le module local des commandes
mod commands;

use raise::{
    raise_error, user_debug, user_error, user_info, user_warn,
    utils::{context, prelude::*, Arc},
};

// NOUVEAUX IMPORTS : Moteur de stockage et Session
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use raise::utils::session::SessionManager;

// ============================================================================
// 🎯 DÉFINITION DU CONTEXTE GLOBAL DU CLI
// ============================================================================
#[derive(Clone)]
pub struct CliContext {
    pub config: &'static AppConfig,
    pub session_mgr: SessionManager,
    pub storage: Arc<StorageEngine>,
}

// ============================================================================

#[derive(Parser)]
#[command(name = "raise-cli")]
#[command(about = "CLI unifié pour la manipulation des modules Raise", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
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
        raise_error!(
            "CLI_CRITICAL_INIT_FAILED",
            error = e,
            context = json!({"step": "AppConfig::init", "hint": "Vérifiez vos fichiers de configuration"})
        );
    }

    // 2. Initialisation du Logger
    context::init_logging();

    // 3. Initialisation de la Langue
    let config = AppConfig::get();
    context::init_i18n(&config.core.language).await?;

    // 4. INITIALISATION DU MOTEUR DE STOCKAGE ET DE SESSION
    let db_root = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: Le chemin PATH_RAISE_DOMAIN est introuvable !");

    let storage = Arc::new(StorageEngine::new(JsonDbConfig::new(db_root)));
    let session_mgr = SessionManager::new(storage.clone());

    // 🎯 5. CRÉATION DU CONTEXTE UNIFIÉ
    let ctx = CliContext {
        config: AppConfig::get(),
        session_mgr,
        storage,
    };

    // 6. AUTO-LOGIN SILENCIEUX STRICT (Tolérance Zéro)
    let username = match ctx.config.user.as_ref() {
        // <-- Utilisation de ctx.config
        Some(user_config) => user_config.id.clone(),
        None => {
            raise_error!(
                "CLI_USER_NOT_FOUND",
                context = json!({
                    "action": "cli_auto_login",
                    "hint": "Aucun utilisateur n'a été résolu par AppConfig. Assurez-vous que votre compte OS existe dans la collection 'users'."
                })
            );
        }
    };

    if let Err(e) = ctx.session_mgr.start_session(&username).await {
        raise_error!(
            "CLI_SESSION_START_FAILED",
            error = e,
            context = json!({
                "user": username,
                "hint": "Le démarrage de la session a échoué. Vérifiez que la base de données système est accessible."
            })
        );
    }

    user_info!(
        "CLI_START_INITIALIZED",
        json!({
            "version": env!("CARGO_PKG_VERSION"),
            "mode": if cfg!(debug_assertions) { "debug" } else { "release" },
            "component": "cli_engine",
            "active_user": username
        })
    );

    // 7. Parsing & Dispatch
    let cli = Cli::parse();

    match cli.command {
        Some(cmd) => {
            if let Err(e) = execute_command(cmd.clone(), ctx.clone()).await {
                raise_error!(
                    "CLI_COMMAND_EXECUTION_FAILED",
                    error = e,
                    context = json!({
                        "command": format!("{:?}", cmd),
                        "trace": "critical_failure"
                    })
                );
            }
        }
        None => {
            run_global_shell(ctx).await?;
        }
    }

    user_debug!("CLI_EXECUTION_FINISHED");
    Ok(())
}

/// Boucle principale du Shell Global (REPL)
async fn run_global_shell(ctx: CliContext) -> RaiseResult<()> {
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

    // 🎯 CORRECTION 1 : On utilise le contexte au lieu d'appeler AppConfig::get()
    let history_path = ctx
        .config
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap()
        .join("_system")
        .join("history.txt");

    let _ = rl.load_history(&history_path);

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
                                    if let Err(e) = execute_command(cmd.clone(), ctx.clone()).await
                                    {
                                        user_error!(
                                            "CLI_COMMAND_EXECUTION_FAILED",
                                            json!({
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
        user_warn!(
            "CLI_HISTORY_SAVE_FAILED",
            json!({
                "error": e.to_string(),
                "path": history_path
            })
        );
    }

    Ok(())
}

async fn execute_command(cmd: Commands, ctx: CliContext) -> RaiseResult<()> {
    match cmd {
        Commands::Workflow(args) => commands::workflow::handle(args, ctx).await,
        Commands::ModelEngine(args) => commands::model_engine::handle(args, ctx).await,
        Commands::Jsondb(args) => commands::jsondb::handle(args, ctx).await,
        Commands::Ai(args) => commands::ai::handle(args, ctx).await,
        Commands::Genetics(args) => commands::genetics::handle(args, ctx).await,
        Commands::Blockchain(args) => commands::blockchain::handle(args, ctx).await,
        Commands::Plugins(args) => commands::plugins::handle(args, ctx).await,
        Commands::Traceability(args) => commands::traceability::handle(args, ctx).await,
        Commands::Spatial(args) => commands::spatial::handle(args, ctx).await,
        Commands::CodeGen(args) => commands::code_gen::handle(args, ctx).await,
        Commands::Validator(args) => commands::validator::handle(args, ctx).await,
        Commands::Utils(args) => commands::utils::handle(args, ctx).await,
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
