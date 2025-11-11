use anyhow::{bail, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use genaptitude::json_db::storage::{file_storage, JsonDbConfig};

#[derive(Parser, Debug)]
#[command(name = "jsondb", about = "CLI JSON-DB GenAptitude")]
struct Cli {
    /// Racine du repo (où se trouve schemas/v1). Par défaut: cwd.
    #[arg(long)]
    repo_root: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    /// Opérations de base de données
    Db {
        #[command(subcommand)]
        action: DbAction,
    },

    /// Collections
    Collection {
        #[command(subcommand)]
        action: CollAction,
    },
}

#[derive(Subcommand, Debug)]
enum DbAction {
    /// Crée une DB: <space> <db>
    Create { space: String, db: String },

    /// Ouvre une DB (vérifie existence): <space> <db>
    Open { space: String, db: String },

    /// Supprime une DB (soft/hard): <space> <db> [--hard]
    Drop {
        space: String,
        db: String,
        #[arg(long)]
        hard: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CollAction {
    /// Crée une collection: <space> <db> <name> --schema <rel-path>
    /// ex: --schema actors/actor.schema.json
    Create {
        space: String,
        db: String,
        name: String,
        #[arg(long)]
        schema: String,
    },
}

fn build_cfg(repo_root_opt: Option<PathBuf>) -> Result<JsonDbConfig> {
    // NB: JsonDbConfig::from_env lira PATH_GENAPTITUDE_DOMAIN
    let repo = match repo_root_opt {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let cfg = JsonDbConfig::from_env(&repo)?;
    // Optionnel: vérifier que PATH_GENAPTITUDE_DOMAIN est bien fourni
    if std::env::var("PATH_GENAPTITUDE_DOMAIN").is_err() {
        bail!("PATH_GENAPTITUDE_DOMAIN non défini (ex: export PATH_GENAPTITUDE_DOMAIN=/home/zair/genaptitude_domain)");
    }
    Ok(cfg)
}

fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let cli = Cli::parse();
    let cfg = build_cfg(cli.repo_root)?;

    match cli.cmd {
        Cmd::Db { action } => match action {
            DbAction::Create { space, db } => {
                file_storage::create_db(&cfg, &space, &db)?;
                println!("✅ DB créée: {}/{}", space, db);
            }
            DbAction::Open { space, db } => {
                let h = file_storage::open_db(&cfg, &space, &db)?;
                println!(
                    "✅ DB ouverte: {}/{} → {}",
                    h.space,
                    h.database,
                    h.root.display()
                );
            }
            DbAction::Drop { space, db, hard } => {
                let mode = if hard {
                    file_storage::DropMode::Hard
                } else {
                    file_storage::DropMode::Soft
                };
                file_storage::drop_db(&cfg, &space, &db, mode)?;
                println!(
                    "✅ DB supprimée ({}) : {}/{}",
                    if hard { "hard" } else { "soft" },
                    space,
                    db
                );
            }
        },

        Cmd::Collection { action } => match action {
            CollAction::Create {
                space,
                db,
                name,
                schema,
            } => {
                // Vérifie d'abord que la DB est ouvrable
                file_storage::open_db(&cfg, &space, &db)?;
                file_storage::create_collection(&cfg, &space, &db, &name, &schema)?;
                println!(
                    "✅ Collection créée: {}/{} :: {} (schema: {})",
                    space, db, name, schema
                );
            }
        },
    }

    Ok(())
}
