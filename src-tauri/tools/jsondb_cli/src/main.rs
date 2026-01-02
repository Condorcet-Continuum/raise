// FICHIER : src-tauri/tools/jsondb_cli/src/main.rs

use anyhow::Result;
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

// Imports RAISE
use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::query::{Query, QueryEngine};
use raise::json_db::storage::{
    file_storage::{self},
    JsonDbConfig, StorageEngine,
};
use raise::json_db::transactions::manager::TransactionManager;
use raise::json_db::transactions::TransactionRequest;

#[derive(Parser)]
#[command(
    name = "jsondb_cli",
    author = "RAISE Team",
    version,
    about = "Outil d'administration pour RAISE JSON-DB"
)]
struct Cli {
    #[arg(short, long, default_value = "default_space")]
    space: String,

    #[arg(short, long, default_value = "default_db")]
    db: String,

    #[arg(long, env = "RAISE_DATA_DIR")]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    // --- DB & COLLECTIONS ---
    CreateDb,
    DropDb {
        #[arg(long, short = 'f')]
        force: bool,
    },
    CreateCollection {
        #[arg(long)]
        name: String,
        #[arg(long)]
        schema: Option<String>,
    },
    DropCollection {
        #[arg(long)]
        name: String,
    },
    ListCollections,

    // --- INDEXES (NOUVEAU) ---
    CreateIndex {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        field: String,
        /// Type d'index : "unique", "hash", "text", "btree"
        #[arg(long, default_value = "hash")]
        kind: String,
    },
    DropIndex {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        field: String,
    },

    // --- DATA ---
    List {
        #[arg(long)]
        collection: String,
    },
    ListAll {
        #[arg(long)]
        collection: String,
    },
    Insert {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        data: String,
    },
    Query {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        offset: Option<usize>,
    },
    Sql {
        #[arg(long)]
        query: String,
    },
    Import {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        path: PathBuf,
    },
    Transaction {
        #[arg(long)]
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let _ = tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .try_init();

    let cli = Cli::parse();

    let root_path = if let Some(r) = cli.root {
        r
    } else {
        let home = dirs::home_dir().unwrap_or(PathBuf::from("."));
        home.join("raise_domain")
    };

    let config = JsonDbConfig {
        data_root: root_path,
    };

    // Auto-bootstrap
    if !matches!(cli.command, Commands::CreateDb | Commands::DropDb { .. })
        && !config.db_root(&cli.space, &cli.db).exists()
    {
        let storage = StorageEngine::new(config.clone());
        let mgr = CollectionsManager::new(&storage, &cli.space, &cli.db);
        let _ = mgr.init_db();
    }

    let storage = StorageEngine::new(config.clone());
    let mgr = CollectionsManager::new(&storage, &cli.space, &cli.db);

    match cli.command {
        // --- DB ---
        Commands::CreateDb => {
            println!("🔨 Création de la base '{}/{}'...", cli.space, cli.db);
            file_storage::create_db(&config, &cli.space, &cli.db)?;
            mgr.init_db()?;
            println!("✅ Base prête.");
        }

        Commands::DropDb { force } => {
            if !force {
                eprintln!("❌ Utilisez --force pour confirmer.");
                return Ok(());
            }
            println!("🗑️ Suppression...");
            file_storage::drop_db(&config, &cli.space, &cli.db, file_storage::DropMode::Hard)?;
            println!("✅ Terminé.");
        }

        // --- COLLECTIONS ---
        Commands::CreateCollection { name, schema } => {
            println!("🚀 Création de '{}'...", name);
            mgr.create_collection(&name, schema)?;
            println!("✅ Collection créée.");
        }

        Commands::DropCollection { name } => {
            println!("🔥 Suppression de la collection '{}'...", name);
            mgr.drop_collection(&name)?;
            println!("✅ Collection supprimée.");
        }

        Commands::ListCollections => {
            let cols = mgr.list_collections()?;
            println!("📂 Collections dans {}/{}:", cli.space, cli.db);
            for c in cols {
                println!("  - {}", c);
            }
        }

        // --- INDEXES (IMPLÉMENTATION) ---
        Commands::CreateIndex {
            collection,
            field,
            kind,
        } => {
            println!(
                "🏗️  Création de l'index '{}' sur {}.{}...",
                kind, collection, field
            );
            // Note: create_index doit être publique dans CollectionsManager
            mgr.create_index(&collection, &field, &kind)?;
            println!("✅ Index créé.");
        }

        Commands::DropIndex { collection, field } => {
            println!("🔥 Suppression de l'index sur {}.{}...", collection, field);
            // Note: drop_index doit être publique dans CollectionsManager
            mgr.drop_index(&collection, &field)?;
            println!("✅ Index supprimé.");
        }

        // --- DATA ---
        Commands::List { collection } => match mgr.list_all(&collection) {
            Ok(docs) => {
                println!(
                    "📄 Collection '{}' ({} documents) :",
                    collection,
                    docs.len()
                );
                for doc in docs {
                    let id = doc.get("id").and_then(|v| v.as_str()).unwrap_or("?");
                    let name_val = doc.get("name");
                    let name = if let Some(s) = name_val.and_then(|v| v.as_str()) {
                        s.to_string()
                    } else {
                        "Objet Complexe".to_string()
                    };
                    println!(" - [{}] {}", id, name);
                }
            }
            Err(e) => eprintln!("❌ Erreur lecture : {}", e),
        },

        Commands::ListAll { collection } => {
            let docs = mgr.list_all(&collection)?;
            println!("--- {} documents dans '{}' ---", docs.len(), collection);
            for doc in docs {
                println!("{}", serde_json::to_string(&doc)?);
            }
        }

        Commands::Insert { collection, data } => {
            let content_str = if let Some(path) = data.strip_prefix('@') {
                fs::read_to_string(path)?
            } else {
                data.clone()
            };
            let json_doc: Value = serde_json::from_str(&content_str)?;

            match mgr.insert_with_schema(&collection, json_doc) {
                Ok(id) => println!("✅ Document inséré : {}", id),
                Err(e) => eprintln!("❌ Erreur d'insertion : {}", e),
            }
        }

        Commands::Query {
            collection,
            filter: _,
            limit,
            offset,
        } => {
            let query = Query {
                collection: collection.clone(),
                filter: None,
                sort: None,
                limit,
                offset,
                projection: None,
            };
            let result = QueryEngine::new(&mgr).execute_query(query).await?;
            println!("🔎 Résultat : {} documents", result.documents.len());
            for doc in result.documents {
                println!("{}", doc);
            }
        }

        Commands::Sql { query } => {
            let q = raise::json_db::query::sql::parse_sql(&query)?;
            let result = QueryEngine::new(&mgr).execute_query(q).await?;
            println!("⚡ SQL Result : {} documents", result.documents.len());
            for doc in result.documents {
                println!("{}", doc);
            }
        }

        Commands::Import { collection, path } => {
            let mut count = 0;
            if path.is_dir() {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    if entry.path().extension().is_some_and(|e| e == "json") {
                        let content = fs::read_to_string(entry.path())?;
                        if let Ok(doc) = serde_json::from_str::<Value>(&content) {
                            mgr.insert_with_schema(&collection, doc)?;
                            count += 1;
                            print!(".");
                        }
                    }
                }
            } else {
                let content = fs::read_to_string(path)?;
                let doc = serde_json::from_str::<Value>(&content)?;
                mgr.insert_with_schema(&collection, doc)?;
                count += 1;
            }
            println!("\n📦 Import terminé : {} documents.", count);
        }

        Commands::Transaction { file } => {
            let content = fs::read_to_string(&file)?;
            #[derive(Deserialize)]
            struct TxWrapper {
                operations: Vec<TransactionRequest>,
            }
            let reqs = if let Ok(w) = serde_json::from_str::<TxWrapper>(&content) {
                w.operations
            } else {
                serde_json::from_str::<Vec<TransactionRequest>>(&content)?
            };
            let tm = TransactionManager::new(&config, &cli.space, &cli.db);
            println!("🔄 Lancement de la transaction intelligente...");
            tm.execute_smart(reqs).await?;
            println!("✅ Transaction exécutée avec succès.");
        }
    }

    Ok(())
}
