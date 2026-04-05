// FICHIER : src-tauri/tools/raise-cli/src/main.rs

use clap::{Parser, Subcommand};

// On garde le module local des commandes
mod commands;

use raise::{
    json_db::{
        jsonld::VocabularyRegistry,
        storage::{JsonDbConfig, StorageEngine},
    },
    raise_error, user_debug, user_error, user_info, user_warn,
    utils::{context, prelude::*},
};

// ============================================================================
// 🎯 DÉFINITION DU CONTEXTE GLOBAL DU CLI
// ============================================================================
#[derive(Clone)]
pub struct CliContext {
    pub config: &'static AppConfig,
    pub session_mgr: context::SessionManager,
    pub storage: SharedRef<StorageEngine>,
    pub active_user: String,
    pub active_domain: String,
    pub active_db: String,
    pub is_test_mode: bool,
    pub is_simulation: bool,
    pub sim_domain: String,
    pub sim_db: String,
}

// ============================================================================

#[derive(Parser)]
#[command(name = "raise-cli")]
#[command(about = "CLI unifié pour la manipulation des modules Raise", long_about = None)]
#[command(version)]
struct Cli {
    // 🆕 Arguments globaux pour la surcharge de contexte
    #[arg(
        long,
        global = true,
        env = "RAISE_USER",
        help = "Surcharge l'utilisateur actif"
    )]
    user: Option<String>,

    #[arg(
        long,
        global = true,
        env = "RAISE_DOMAIN",
        help = "Surcharge le domaine par défaut"
    )]
    domain: Option<String>,

    #[arg(
        long,
        global = true,
        env = "RAISE_DB",
        help = "Surcharge la base de données par défaut"
    )]
    db: Option<String>,

    #[arg(
        long,
        global = true,
        env = "RAISE_SIMULATE",
        help = "Active le mode Bac à Sable (Simulation IA)"
    )]
    simulate: bool,

    #[arg(
        long,
        global = true,
        help = "Surcharge le domaine cible pour la simulation"
    )]
    sim_domain: Option<String>,

    #[arg(
        long,
        global = true,
        help = "Surcharge la base de données cible pour la simulation"
    )]
    sim_db: Option<String>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Clone, Debug)]
enum Commands {
    Workflow(commands::workflow::WorkflowArgs),
    ModelEngine(commands::model_engine::ModelArgs),
    Rules(commands::rules::RulesArgs),
    Jsondb(commands::jsondb::JsondbArgs),
    Ai(commands::ai::AiArgs),
    Dl(commands::dl::DlArgs),
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
            context = json_value!({"step": "AppConfig::init"})
        );
    }

    // 2. Initialisation du Logger et Langue
    context::init_logging();
    let config = AppConfig::get();
    context::init_i18n(&config.core.language).await?;

    // 🎯 3. PARSING DU CLI LE PLUS TÔT POSSIBLE (Inversion de contrôle)
    let cli = Cli::parse();

    // 🎯 4. RÉSOLUTION SÉMANTIQUE DES PRIORITÉS (CLI > Config > Fallback)
    let active_user = cli.user.clone().unwrap_or_else(|| {
        config
            .user
            .as_ref()
            .map(|u| u.id.clone())
            .unwrap_or_else(|| {
                // Tolérance zéro évitée ici, on gérera l'erreur à la session si besoin
                "unknown_user".to_string()
            })
    });

    let active_domain = cli.domain.clone().unwrap_or_else(|| {
        config
            .user
            .as_ref()
            .and_then(|u| u.default_domain.clone())
            .unwrap_or_else(|| config.system_domain.clone())
    });

    let active_db = cli.db.clone().unwrap_or_else(|| {
        config
            .user
            .as_ref()
            .and_then(|u| u.default_db.clone())
            .unwrap_or_else(|| config.system_db.clone())
    });

    // 5. INITIALISATION DU MOTEUR DE STOCKAGE ET DE SESSION
    let db_root = config
        .get_path("PATH_RAISE_DOMAIN")
        .expect("ERREUR: PATH_RAISE_DOMAIN manquant !");

    // Note: Si StorageEngine a besoin de active_domain plus tard, c'est ici qu'on le passera.
    let storage = SharedRef::new(StorageEngine::new(JsonDbConfig::new(db_root)));
    let ontology_path = storage.config.data_root.join("_system/ontology");
    bootstrap_semantic_engine(&ontology_path).await?;
    let session_mgr = context::SessionManager::new(storage.clone());

    // --- RÉSOLUTION DU CONTEXTE DE SIMULATION ---
    let is_simulation = cli.simulate;

    // Idéalement, config.simulation_context devrait exister dans ta structure AppConfig Rust.
    // Pour l'instant, utilisons des valeurs par défaut robustes si tu n'as pas encore mis à jour
    // le parseur serde_json de AppConfig.
    let sim_domain = cli.sim_domain.unwrap_or_else(|| "sim_mbse2".to_string());
    let sim_db = cli.sim_db.unwrap_or_else(|| "sim_raise".to_string());

    // 6. CRÉATION DU CONTEXTE UNIFIÉ
    let ctx = CliContext {
        config,
        session_mgr,
        storage,
        active_user: active_user.clone(),
        active_domain,
        active_db,
        is_test_mode: false,
        is_simulation,
        sim_domain,
        sim_db,
    };

    // 7. AUTO-LOGIN AVEC L'UTILISATEUR RÉSOLU
    if active_user == "unknown_user" {
        user_warn!(
            "CLI_GHOST_MODE",
            json_value!({"hint": "Aucun utilisateur résolu. Le CLI démarre en mode restreint (Setup)."})
        );
    } else {
        match ctx.session_mgr.start_session(&active_user).await {
            Ok(_) => {
                user_info!(
                    "CLI_START_INITIALIZED",
                    json_value!({
                        "version": env!("CARGO_PKG_VERSION"),
                        "active_user": ctx.active_user,
                        "active_domain": ctx.active_domain,
                        "active_db": ctx.active_db
                    })
                );
            }
            Err(e) => {
                user_warn!(
                    "CLI_SESSION_UNAVAILABLE",
                    json_value!({
                        "user": active_user,
                        "technical_error": e.to_string(),
                        "hint": "Si le système est vierge, utilisez 'jsondb create-db' pour l'initialiser."
                    })
                );
            }
        }
    }

    if let Err(e) = ctx.session_mgr.start_session(&active_user).await {
        raise_error!(
            "CLI_SESSION_START_FAILED",
            error = e,
            context = json_value!({"user": active_user})
        );
    }

    user_info!(
        "CLI_START_INITIALIZED",
        json_value!({
            "version": env!("CARGO_PKG_VERSION"),
            "active_user": ctx.active_user,
            "active_domain": ctx.active_domain,
            "active_db": ctx.active_db
        })
    );

    // 8. DISPATCH DES COMMANDES
    match cli.command {
        Some(cmd) => {
            if let Err(e) = execute_command(cmd.clone(), ctx.clone()).await {
                raise_error!(
                    "CLI_COMMAND_EXECUTION_FAILED",
                    error = e,
                    context = json_value!({"command": format!("{:?}", cmd)})
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
async fn run_global_shell(mut ctx: CliContext) -> RaiseResult<()> {
    use rustyline::error::ReadlineError;
    use rustyline::DefaultEditor;

    println!("--------------------------------------------------");
    println!("🚀 RAISE GLOBAL SHELL - v{}", env!("CARGO_PKG_VERSION"));
    println!("👤 User   : {}", ctx.active_user);
    println!("🌍 Domain : {}", ctx.active_domain);
    println!("🗄️  DB     : {}", ctx.active_db);
    println!("   Tapez 'help' pour la liste des commandes.");
    println!("   Tapez 'exit' ou 'quit' pour quitter.");
    println!("--------------------------------------------------");

    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            raise_error!(
                "CLI_EDITOR_INIT_FAILED",
                error = e,
                context = json_value!({
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
        // 🎯 1. AUTO-SYNC : On demande au noyau la vérité absolue avant d'afficher le prompt
        if let Some(session) = ctx.session_mgr.get_current_session().await {
            ctx.active_user = session.user_handle.clone();
            ctx.active_domain = session.context.current_domain.clone();
            ctx.active_db = session.context.current_db.clone();
        }
        let prompt = format!(
            "RAISE [{}@{}/{}]> ",
            ctx.active_user, ctx.active_domain, ctx.active_db
        );

        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                let mut input = line.trim().to_string();
                if input.is_empty() {
                    continue;
                }
                let _ = rl.add_history_entry(input.as_str());

                // 🛡️ COMMANDES SYSTÈMES DU SHELL
                if input == "exit" || input == "quit" {
                    println!("👋 Au revoir !");
                    break;
                } else if input == "clear" {
                    print!("\x1B[2J\x1B[1;1H");
                    continue;
                }
                // 🎯 2. ALIAS UX : On traduit les commandes courtes pour Clap (utils.rs)
                if input.starts_with("login ")
                    || input.starts_with("config")
                    || input.starts_with("use-domain ")
                    || input.starts_with("use-db ")
                {
                    input = format!("utils {}", input);
                } else if input.starts_with("logout") {
                    input = "utils logout".to_string();
                }

                // 🔄 DISPATCH CLAP STANDARD
                match shell_words::split(&input) {
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
                                            json_value!({
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
                    json_value!({
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
            json_value!({
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
        Commands::Rules(args) => commands::rules::handle(args, ctx).await,
        Commands::Jsondb(args) => commands::jsondb::handle(args, ctx).await,
        Commands::Ai(args) => commands::ai::handle(args, ctx).await,
        Commands::Dl(args) => commands::dl::handle(args, ctx).await,
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

/// Initialise le registre sémantique global (VocabularyRegistry).
async fn bootstrap_semantic_engine(ontology_path: &Path) -> RaiseResult<()> {
    // 1. Vérification physique du dossier
    if !ontology_path.exists() {
        raise_error!(
            "ERR_SEMANTIC_BOOTSTRAP",
            error = "Dossier d'ontologie introuvable",
            context = json_value!({
                "path": ontology_path.to_string_lossy(),
                "hint": "Le dossier '_system/ontology' est indispensable au fonctionnement du CLI."
            })
        );
    }

    let init_result = VocabularyRegistry::init(ontology_path).await;

    if let Err(e) = init_result {
        raise_error!(
            "ERR_VOCABULARY_INIT_FAILED",
            error = e.to_string(),
            context = json_value!({ "path": ontology_path.to_string_lossy() })
        );
    }

    user_info!(
        "SEMANTIC_ENGINE_READY",
        json_value!({ "path": ontology_path })
    );
    Ok(())
}

#[cfg(test)]
impl CliContext {
    pub fn mock(
        config: &'static AppConfig,
        session_mgr: context::SessionManager,
        storage: SharedRef<StorageEngine>,
    ) -> Self {
        Self {
            config,
            session_mgr,
            storage,
            active_user: "mock_user".to_string(),
            active_domain: "mock_domain".to_string(),
            active_db: "mock_db".to_string(),
            is_test_mode: true,
            is_simulation: false, // Faux par défaut dans les tests
            sim_domain: "mock_sim_domain".to_string(),
            sim_db: "mock_sim_db".to_string(),
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
