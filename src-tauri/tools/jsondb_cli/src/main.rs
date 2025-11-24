use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};

// Assurez-vous que 'genaptitude' est bien le nom de votre crate principal
use genaptitude::json_db::{
    collections::manager::CollectionsManager,
    query::parser::parse_sort_specs,
    query::{Query, QueryEngine, SortField, SortOrder},
    storage::{file_storage, JsonDbConfig},
};

/// CLI JSON-DB GenAptitude
#[derive(Parser, Debug)]
#[command(name = "jsondb_cli", about = "CLI JSON-DB GenAptitude")]
struct Cli {
    /// Racine du repo (où se trouve schemas/v1). Par défaut: cwd.
    #[arg(long)]
    repo_root: Option<PathBuf>,

    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand, Debug)]
enum Cmd {
    Usage,
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

    /// Documents (insert/upsert à partir d'un fichier JSON, avec schéma)
    Document {
        #[command(subcommand)]
        action: DocAction,
    },
    /// Requêtes complexes (JSON Query, avec filtres, tri, limites)
    Query {
        #[command(subcommand)]
        action: QueryAction,
    },

    /// Requêtes SQL (placeholder pour une future implémentation)
    Sql {
        #[command(subcommand)]
        action: SqlAction,
    },

    /// Seeding d'une DB à partir de fichiers dataset
    Dataset {
        #[command(subcommand)]
        action: DatasetAction,
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

    /// Requête sur une collection
    Query {
        /// Espace logique (ex: un2)
        space: String,
        /// Nom de la DB (ex: _system)
        db: String,
        /// Nom de la collection (ex: articles)
        collection: String,

        /// Filtre JSON pour QueryFilter
        #[arg(long)]
        filter_json: Option<String>,

        /// Spécifications de tri (répétables) : --sort createdAt:desc
        #[arg(long = "sort")]
        sort: Vec<String>,

        /// Décalage (skip) optionnel
        #[arg(long)]
        offset: Option<usize>,

        /// Limite du nombre de résultats
        #[arg(long)]
        limit: Option<usize>,

        /// Si présent, équivalent à --sort createdAt:desc
        #[arg(long)]
        latest: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CollAction {
    /// Crée une collection: <space> <db> <name> --schema <rel-path>
    Create {
        space: String,
        db: String,
        name: String,
        #[arg(long)]
        schema: String,
    },
}

#[derive(Subcommand, Debug)]
enum DocAction {
    /// Insert: échoue si un document avec le même id existe déjà
    Insert {
        space: String,
        db: String,
        #[arg(long)]
        schema: String,
        #[arg(long)]
        file: PathBuf,
    },

    /// Upsert: insert si nouveau, sinon update
    Upsert {
        space: String,
        db: String,
        #[arg(long)]
        schema: String,
        #[arg(long)]
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum QueryAction {
    /// Exécute une requête complexe basée sur un fichier JSON
    FindMany {
        space: String,
        db: String,
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum SqlAction {
    Exec {
        space: String,
        db: String,
        query: String,
    },
}

#[derive(Subcommand, Debug)]
enum DatasetAction {
    /// Insère tous les documents JSON d'un dossier
    SeedDir {
        space: String,
        db: String,
        dataset_rel_dir: PathBuf,
    },
}

/// Construction de la config JSON-DB à partir de l'env + repo_root
fn build_cfg(repo_root_opt: Option<PathBuf>) -> Result<JsonDbConfig> {
    let repo = match repo_root_opt {
        Some(p) => p,
        None => std::env::current_dir()?,
    };
    let cfg = JsonDbConfig::from_env(&repo)?;
    if std::env::var("PATH_GENAPTITUDE_DOMAIN").is_err() {
        bail!("PATH_GENAPTITUDE_DOMAIN non défini");
    }
    Ok(cfg)
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

// Fonction d'aide privée : logique pour insérer tous les fichiers d'un dossier.
fn run_seed_dir(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    dataset_rel_dir: &PathBuf,
) -> Result<()> {
    // 1. Instancier le manager de collections
    let mgr = CollectionsManager::new(&cfg, &space, &db);

    // 2. Déterminer le chemin du dataset
    // CORRECTION: On utilise directement le chemin fourni par l'utilisateur (relatif au CWD)
    // au lieu de chercher une méthode .dataset_path() qui n'existe pas sur la config.
    let abs_dataset_dir = if dataset_rel_dir.is_absolute() {
        dataset_rel_dir.clone()
    } else {
        std::env::current_dir()?.join(dataset_rel_dir)
    };

    // 3. Déterminer la collection et le schéma à partir du nom du dossier
    let collection = dataset_rel_dir
        .file_name()
        .and_then(|s| s.to_str())
        .context("Le chemin de dataset doit finir par un nom de collection (ex: articles)")?;

    // Infère le chemin de schéma relatif (ex: articles/article.schema.json)
    let schema_rel = format!("{collection}/{collection}.schema.json");

    println!(
        "🌱 Démarrage du seeding pour collection '{}':\n - Dossier: {}\n - Schéma: {}",
        collection,
        abs_dataset_dir.display(),
        schema_rel
    );

    if !abs_dataset_dir.exists() {
        bail!(
            "Le dossier dataset n'existe pas : {}",
            abs_dataset_dir.display()
        );
    }

    // 4. Itérer et insérer
    let mut count = 0;
    for entry in fs::read_dir(&abs_dataset_dir).with_context(|| {
        format!(
            "Impossible de lire le dossier dataset: {}",
            abs_dataset_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            let rd = File::open(&path).with_context(|| format!("Ouverture {}", path.display()))?;

            let doc: Value = match serde_json::from_reader(rd) {
                Ok(d) => d,
                Err(e) => {
                    eprintln!("\n⚠️ Erreur JSON invalide dans {}: {}\n", path.display(), e);
                    continue;
                }
            };

            match mgr.insert_with_schema(&schema_rel, doc) {
                Ok(_stored) => {
                    count += 1;
                    print!(".");
                    std::io::stdout().flush()?;
                }
                Err(e) => {
                    eprintln!("\n❌ Échec de l'insertion pour {}: {}\n", path.display(), e);
                }
            }
        }
    }
    println!(
        "\n✅ Inséré {} document(s) dans la collection '{}'.",
        count, collection
    );

    Ok(())
}

fn usages() {
    println!(
        r#"
Usage: jsondb_cli <COMMAND> [OPTIONS]

-- COMMANDES DE BASE DE DONNÉES (Db) --------------------------------------------
jsondb_cli db create <space> <db>
jsondb_cli db open <space> <db>
jsondb_cli db drop <space> <db> --hard

-- COMMANDES DE COLLECTIONS (Collection) ---------------------------------------
jsondb_cli collection create <space> <db> <name> <schema> 
jsondb_cli collection drop <space> <db> <name> --hard

-- COMMANDES DE DOCUMENTS (Document) -------------------------------------------
jsondb_cli document insert <space> <db> <schema> <file>
jsondb_cli document upsert <space> <db> <schema> <file>

-- COMMANDES DE DATASET (Dataset) ----------------------------------------------
jsondb_cli dataset seed-dir <space> <db> <dataset_dir_rel>

-- COMMANDES DE REQUÊTES (Query / Sql) -----------------------------------------
jsondb_cli query find-many <space> <db> <file_query_json>
jsondb_cli sql exec <space> <db> "<SQL_QUERY>"

-- OPTIONS GLOBALES ------------------------------------------------------------
jsondb_cli --repo-root /path/to/repo <COMMAND> ...
"#
    );
}

#[tokio::main]
async fn main() -> Result<()> {
    // Charge .env à la racine du repo courant si présent
    dotenvy::dotenv().ok();
    init_tracing();

    let cli = Cli::parse();
    let cfg = build_cfg(cli.repo_root.clone())?;

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
            DbAction::Query {
                space,
                db,
                collection,
                filter_json,
                sort,
                offset,
                limit,
                latest,
            } => {
                file_storage::open_db(&cfg, &space, &db)?;
                let mgr = CollectionsManager::new(&cfg, &space, &db);
                let engine = QueryEngine::new(&mgr);

                let filter = if let Some(raw) = filter_json {
                    let v: Value = serde_json::from_str(&raw)
                        .with_context(|| format!("Parse du filtre JSON: {raw}"))?;
                    Some(serde_json::from_value(v)?)
                } else {
                    None
                };

                let mut sort_fields: Vec<SortField> = Vec::new();
                if !sort.is_empty() {
                    sort_fields = parse_sort_specs(&sort)
                        .map_err(|e| anyhow!("Spécification de tri invalide: {e}"))?;
                } else if latest {
                    sort_fields.push(SortField {
                        field: "createdAt".to_string(),
                        order: SortOrder::Desc,
                    });
                }

                let q = Query {
                    collection: collection.clone(),
                    filter,
                    sort: if sort_fields.is_empty() {
                        None
                    } else {
                        Some(sort_fields)
                    },
                    offset,
                    limit,
                    projection: None,
                };

                let result = engine
                    .execute_query(q)
                    .await
                    .with_context(|| "Exécution de la requête")?;

                if result.documents.is_empty() {
                    println!("(aucun document)");
                } else {
                    for doc in result.documents {
                        println!("{}", serde_json::to_string_pretty(&doc)?);
                        println!("---");
                    }
                }
            }
        },
        Cmd::Collection { action } => match action {
            CollAction::Create {
                space,
                db,
                name,
                schema,
            } => {
                file_storage::open_db(&cfg, &space, &db)?;
                file_storage::create_collection(&cfg, &space, &db, &name, &schema)?;
                println!(
                    "✅ Collection créée: {}/{} :: {} (schema: {})",
                    space, db, name, schema
                );
            }
        },
        Cmd::Document { action } => match action {
            DocAction::Insert {
                space,
                db,
                schema,
                file,
            } => {
                file_storage::open_db(&cfg, &space, &db)?;
                let mgr = CollectionsManager::new(&cfg, &space, &db);
                let rd = File::open(&file)?;
                let doc: Value = serde_json::from_reader(rd)?;
                let stored = mgr.insert_with_schema(&schema, doc)?;
                let id = stored
                    .get("id")
                    .or(stored.get("_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                println!("✅ Inserted: {}", id);
            }
            DocAction::Upsert {
                space,
                db,
                schema,
                file,
            } => {
                file_storage::open_db(&cfg, &space, &db)?;
                let mgr = CollectionsManager::new(&cfg, &space, &db);
                let rd = File::open(&file)?;
                let doc: Value = serde_json::from_reader(rd)?;
                let stored = mgr.upsert_with_schema(&schema, doc)?;
                let id = stored
                    .get("id")
                    .or(stored.get("_id"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("?");
                println!("✅ Upserted: {}", id);
            }
        },
        Cmd::Query { action } => match action {
            QueryAction::FindMany { space, db, file } => {
                file_storage::open_db(&cfg, &space, &db)?;
                let mgr = CollectionsManager::new(&cfg, &space, &db);
                let engine = QueryEngine::new(&mgr);

                let rd = File::open(&file)?;
                let query: Query = serde_json::from_reader(rd)?;
                println!("🔎 Requête chargée:\n{:#?}", query);

                let result = engine.execute_query(query).await?;
                println!("✅ Trouvé {} document(s).", result.documents.len());
                for doc in result.documents.iter().take(5) {
                    println!(
                        "   - ID: {}",
                        doc.get("id").or(doc.get("_id")).unwrap_or(&Value::Null)
                    );
                }
            }
        },
        Cmd::Sql { action: _ } => {
            println!("⚠️ Commande SQL non implémentée.");
        }
        Cmd::Dataset { action } => match action {
            DatasetAction::SeedDir {
                space,
                db,
                dataset_rel_dir,
            } => {
                file_storage::open_db(&cfg, &space, &db)?;
                run_seed_dir(&cfg, &space, &db, &dataset_rel_dir)?;
            }
        },
        Cmd::Usage => usages(),
    }

    Ok(())
}
