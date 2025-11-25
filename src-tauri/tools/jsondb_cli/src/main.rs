use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::Value;
use std::fs::{self, File};
use std::io::Write;
use std::path::PathBuf;
use tracing_subscriber::{fmt, EnvFilter};
use uuid::Uuid;

use genaptitude::json_db::{
    collections::manager::CollectionsManager,
    query::parser::parse_sort_specs,
    query::{Query, QueryEngine, SortField, SortOrder},
    storage::{file_storage, JsonDbConfig},
    transactions::TransactionManager,
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
    Db {
        #[command(subcommand)]
        action: DbAction,
    },
    Collection {
        #[command(subcommand)]
        action: CollAction,
    },
    Document {
        #[command(subcommand)]
        action: DocAction,
    },
    Query {
        #[command(subcommand)]
        action: QueryAction,
    },
    Transaction {
        #[command(subcommand)]
        action: TransactionAction,
    },
    Sql {
        #[command(subcommand)]
        action: SqlAction,
    },
    Dataset {
        #[command(subcommand)]
        action: DatasetAction,
    },
}

#[derive(Subcommand, Debug)]
enum DbAction {
    Create {
        space: String,
        db: String,
    },
    Open {
        space: String,
        db: String,
    },
    Drop {
        space: String,
        db: String,
        #[arg(long)]
        hard: bool,
    },
    Query {
        space: String,
        db: String,
        collection: String,
        #[arg(long)]
        filter_json: Option<String>,
        #[arg(long = "sort")]
        sort: Vec<String>,
        #[arg(long)]
        offset: Option<usize>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        latest: bool,
    },
}

#[derive(Subcommand, Debug)]
enum CollAction {
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
    Insert {
        space: String,
        db: String,
        #[arg(long)]
        schema: String,
        #[arg(long)]
        file: PathBuf,
    },
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
    FindMany {
        space: String,
        db: String,
        file: PathBuf,
    },
}

#[derive(Subcommand, Debug)]
enum TransactionAction {
    Execute {
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
    SeedDir {
        space: String,
        db: String,
        dataset_rel_dir: PathBuf,
    },
}

// --- Structures Transaction ---

#[derive(Deserialize, Debug)]
struct CliTransactionRequest {
    operations: Vec<CliOperationRequest>,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", rename_all = "camelCase")]
enum CliOperationRequest {
    Insert { collection: String, doc: Value },
    // Ajout de InsertFrom pour charger depuis un fichier
    InsertFrom { collection: String, path: String },
    Update { collection: String, doc: Value },
    Delete { collection: String, id: String },
}

// --- Helpers ---

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

/// Remplace les variables d'environnement dans le chemin ($HOME, $PATH_GENAPTITUDE_DATASET)
fn expand_path(path: &str) -> PathBuf {
    let mut p = path.to_string();
    if p.contains("$HOME") {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        p = p.replace("$HOME", &home);
    }
    if p.contains("$PATH_GENAPTITUDE_DATASET") {
        let ds = std::env::var("PATH_GENAPTITUDE_DATASET").unwrap_or_else(|_| ".".to_string());
        p = p.replace("$PATH_GENAPTITUDE_DATASET", &ds);
    }
    PathBuf::from(p)
}

/// Génère ou récupère l'ID et l'injecte dans le document
fn ensure_id(doc: &mut Value) -> String {
    let id = match doc.get("id").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => Uuid::new_v4().to_string(), // Utilisation correcte de l'import Uuid
    };
    if let Some(obj) = doc.as_object_mut() {
        obj.insert("id".to_string(), Value::String(id.clone()));
    }
    id
}

// ... (La fonction run_seed_dir et usages() restent inchangés, je les omets pour la concision) ...
// Tu peux les garder tels quels dans ton fichier.
fn run_seed_dir(
    cfg: &JsonDbConfig,
    space: &str,
    db: &str,
    dataset_rel_dir: &PathBuf,
) -> Result<()> {
    let mgr = CollectionsManager::new(cfg, space, db);
    let abs_dataset_dir = if dataset_rel_dir.is_absolute() {
        dataset_rel_dir.clone()
    } else {
        std::env::current_dir()?.join(dataset_rel_dir)
    };
    let collection = dataset_rel_dir
        .file_name()
        .and_then(|s| s.to_str())
        .context("Dataset dir must end with collection name")?;
    let schema_rel = format!("{collection}/{collection}.schema.json");

    println!(
        "🌱 Seeding '{}' from {}",
        collection,
        abs_dataset_dir.display()
    );
    if !abs_dataset_dir.exists() {
        bail!("Dataset dir not found: {}", abs_dataset_dir.display());
    }

    let mut count = 0;
    for entry in fs::read_dir(&abs_dataset_dir)? {
        let path = entry?.path();
        if path.is_file() && path.extension().map_or(false, |s| s == "json") {
            let rd = File::open(&path)?;
            let doc: Value = match serde_json::from_reader(rd) {
                Ok(d) => d,
                Err(_) => continue,
            };
            if mgr.insert_with_schema(&schema_rel, doc).is_ok() {
                count += 1;
                print!(".");
                std::io::stdout().flush()?;
            }
        }
    }
    println!("\n✅ Inséré {} documents.", count);
    Ok(())
}

fn usages() {
    println!(r#"Usage: jsondb_cli <COMMAND> [OPTIONS] ... (voir --help pour détails)"#);
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    init_tracing();

    let cli = Cli::parse();
    let cfg = build_cfg(cli.repo_root.clone())?;

    match cli.cmd {
        // ... (Les commandes Db, Collection, Document, Query, Sql, Dataset restent inchangées) ...
        // Je les reprends pour la complétude
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
                println!("✅ DB supprimée.");
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
                    let v: Value = serde_json::from_str(&raw).context("Parse filter JSON")?;
                    Some(serde_json::from_value(v)?)
                } else {
                    None
                };

                let mut sort_fields = Vec::new();
                if !sort.is_empty() {
                    sort_fields =
                        parse_sort_specs(&sort).map_err(|e| anyhow!("Sort spec invalid: {e}"))?;
                } else if latest {
                    sort_fields.push(SortField {
                        field: "createdAt".to_string(),
                        order: SortOrder::Desc,
                    });
                }

                let q = Query {
                    collection,
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
                let result = engine.execute_query(q).await.context("Execute query")?;

                if result.documents.is_empty() {
                    println!("(aucun document)");
                } else {
                    for doc in result.documents {
                        println!("{}\n---", serde_json::to_string_pretty(&doc)?);
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
                println!("✅ Collection créée: {}", name);
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
                let doc: Value = serde_json::from_reader(File::open(file)?)?;
                let stored = mgr.insert_with_schema(&schema, doc)?;
                println!(
                    "✅ Inserted: {}",
                    stored.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
            DocAction::Upsert {
                space,
                db,
                schema,
                file,
            } => {
                file_storage::open_db(&cfg, &space, &db)?;
                let mgr = CollectionsManager::new(&cfg, &space, &db);
                let doc: Value = serde_json::from_reader(File::open(file)?)?;
                let stored = mgr.upsert_with_schema(&schema, doc)?;
                println!(
                    "✅ Upserted: {}",
                    stored.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                );
            }
        },
        Cmd::Query { action } => match action {
            QueryAction::FindMany { space, db, file } => {
                file_storage::open_db(&cfg, &space, &db)?;
                let mgr = CollectionsManager::new(&cfg, &space, &db);
                let engine = QueryEngine::new(&mgr);
                let query: Query = serde_json::from_reader(File::open(file)?)?;
                let result = engine.execute_query(query).await?;
                println!("✅ {} document(s) found.", result.documents.len());
                for doc in result.documents.iter().take(5) {
                    println!(
                        "   - ID: {}",
                        doc.get("id").and_then(|v| v.as_str()).unwrap_or("?")
                    );
                }
            }
        },
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
        Cmd::Sql { action: _ } => println!("⚠️ SQL not implemented"),
        Cmd::Usage => usages(),

        // --- MISE À JOUR IMPORTANTE ---
        Cmd::Transaction { action } => match action {
            TransactionAction::Execute { space, db, file } => {
                // 1. CHARGEMENT ENV & CONFIG
                // On s'assure que le .env est chargé pour avoir accès à PATH_GENAPTITUDE_DATASET
                file_storage::open_db(&cfg, &space, &db)?;
                let tm = TransactionManager::new(&cfg, &space, &db);

                // 2. EXPANSION DU CHEMIN DU FICHIER DE TRANSACTION
                // C'est ici qu'on permet l'utilisation de $PATH_GENAPTITUDE_DATASET dans l'argument CLI
                let file_str = file.to_string_lossy();
                let expanded_file_path = expand_path(&file_str);

                println!(
                    "📂 Lecture du fichier de transaction : {:?}",
                    expanded_file_path
                );

                let rd = File::open(&expanded_file_path).with_context(|| {
                    format!("Impossible d'ouvrir le fichier : {:?}", expanded_file_path)
                })?;

                let req: CliTransactionRequest = serde_json::from_reader(rd)
                    .with_context(|| "Erreur de parsing du JSON de transaction")?;

                println!(
                    "🔄 Exécution de la transaction ({} opérations)...",
                    req.operations.len()
                );

                tm.execute(|tx| {
                    for op in req.operations {
                        match op {
                            CliOperationRequest::Insert {
                                collection,
                                mut doc,
                            } => {
                                let id = ensure_id(&mut doc);
                                tx.add_insert(&collection, &id, doc);
                                println!(" + Insert: {}/{}", collection, id);
                            }
                            CliOperationRequest::InsertFrom { collection, path } => {
                                // Expansion également pour les chemins DANS le fichier JSON
                                let expanded_path = expand_path(&path);
                                let content = fs::read_to_string(&expanded_path)
                                    .with_context(|| {
                                        format!("Lecture du fichier source : {:?}", expanded_path)
                                    })
                                    .map_err(|e| anyhow::anyhow!(e))?;

                                let mut doc: Value = serde_json::from_str(&content)
                                    .with_context(|| {
                                        format!("Parsing du fichier source : {:?}", expanded_path)
                                    })
                                    .map_err(|e| anyhow::anyhow!(e))?;

                                let id = ensure_id(&mut doc);
                                tx.add_insert(&collection, &id, doc);
                                println!(
                                    " + Insert (From File): {}/{} (src: {:?})",
                                    collection, id, expanded_path
                                );
                            }
                            CliOperationRequest::Update { collection, doc } => {
                                let id = doc
                                    .get("id")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| anyhow!("Missing id for update"))?
                                    .to_string();
                                tx.add_update(&collection, &id, None, doc);
                                println!(" + Update: {}/{}", collection, id);
                            }
                            CliOperationRequest::Delete { collection, id } => {
                                tx.add_delete(&collection, &id, None);
                                println!(" - Delete: {}/{}", collection, id);
                            }
                        }
                    }
                    Ok(())
                })?;

                println!("✅ Transaction validée (Commit ACID) !");
            }
        },
    }

    Ok(())
}
