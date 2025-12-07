// FICHIER : src-tauri/tools/jsondb_cli/src/main.rs

use anyhow::{anyhow, Result}; // "Context" retiré
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use serde::Deserialize;
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::path::PathBuf;

// Imports GenAptitude
use genaptitude::json_db::collections::manager::CollectionsManager;
use genaptitude::json_db::query::{Query, QueryEngine};
use genaptitude::json_db::storage::{
    file_storage::{self, DropMode},
    JsonDbConfig, StorageEngine,
};
use genaptitude::json_db::transactions::manager::TransactionManager;
use genaptitude::json_db::transactions::TransactionRequest;

#[derive(Parser)]
#[command(
    name = "jsondb_cli",
    author = "GenAptitude Team",
    version,
    about = "Outil d'administration pour GenAptitude JSON-DB"
)]
struct Cli {
    #[arg(short, long, default_value = "default_space")]
    space: String,

    #[arg(short, long, default_value = "default_db")]
    db: String,

    #[arg(long, env = "PATH_GENAPTITUDE_DOMAIN", help = "Dossier racine")]
    root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    CreateDb,
    DropDb {
        #[arg(long, short = 'f')]
        force: bool,
    },
    CreateCollection {
        name: String,
        #[arg(long)]
        schema: Option<String>,
    },
    ListCollections,
    ListAll {
        collection: String,
    },
    Insert {
        collection: String,
        data: String,
    },
    Query {
        collection: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long)]
        limit: Option<usize>,
        #[arg(long)]
        offset: Option<usize>,
    },
    Sql {
        query: String,
    },
    Import {
        collection: String,
        path: PathBuf,
    },
    Transaction {
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    let cli = Cli::parse();

    let config = JsonDbConfig {
        data_root: cli.root.clone(),
    };

    if !matches!(cli.command, Commands::CreateDb | Commands::DropDb { .. }) {
        if !config.db_root(&cli.space, &cli.db).exists() {
            file_storage::create_db(&config, &cli.space, &cli.db)?;
        }
    }

    let storage = StorageEngine::new(config.clone());
    let mgr = CollectionsManager::new(&storage, &cli.space, &cli.db);

    match cli.command {
        // --- GESTION DB ---
        Commands::CreateDb => {
            println!("🔨 Création de la base '{}/{}'...", cli.space, cli.db);
            let schema_rel_path = env::var("GENAPTITUDE_DB_SCHEMA")
                .map_err(|_| anyhow!("❌ Variable ENV 'GENAPTITUDE_DB_SCHEMA' manquante"))?;

            file_storage::create_db(&config, &cli.space, &cli.db)?;

            let schema_path = config.db_root(&cli.space, &cli.db).join(&schema_rel_path);
            if !schema_path.exists() {
                return Err(anyhow!(
                    "CRITIQUE: Schéma maître introuvable : {:?}",
                    schema_path
                ));
            }

            let index_file_path = config.db_root(&cli.space, &cli.db).join("_system.json");

            let system_index: Value = if !index_file_path.exists() {
                println!("📄 Génération de _system.json...");
                let content = fs::read_to_string(&schema_path)?;
                let schema_json: Value = serde_json::from_str(&content)?;

                let mut idx = schema_json
                    .get("examples")
                    .and_then(|ex| ex.as_array())
                    .and_then(|arr| arr.first())
                    .cloned()
                    .ok_or_else(|| anyhow!("Aucun 'examples' trouvé dans index.schema.json"))?;

                if let Some(obj) = idx.as_object_mut() {
                    obj.insert("space".to_string(), json!(cli.space));
                    obj.insert("database".to_string(), json!(cli.db));
                    obj.insert("$schema".to_string(), json!(schema_rel_path));
                }
                fs::write(&index_file_path, serde_json::to_string_pretty(&idx)?)?;
                idx
            } else {
                println!("ℹ️  Lecture de l'index existant...");
                let content = fs::read_to_string(&index_file_path)?;
                // CORRECTION ICI : Type explicit ::<Value>
                serde_json::from_str::<Value>(&content)?
            };

            if let Some(collections) = system_index.get("collections").and_then(|c| c.as_object()) {
                println!("📂 Initialisation des collections définies dans l'index :");
                for (col_name, col_def) in collections {
                    let rel_schema = col_def.get("schema").and_then(|s| s.as_str());
                    let abs_uri = rel_schema
                        .map(|s| format!("db://{}/{}/schemas/v1/{}", cli.space, cli.db, s));
                    print!("   - {} ... ", col_name);
                    match mgr.create_collection(col_name, abs_uri) {
                        Ok(_) => println!("OK"),
                        Err(e) => println!("Erreur ({})", e),
                    }
                }
            }
            println!("✅ Base de données prête.");
        }

        Commands::DropDb { force } => {
            let mode = if force {
                DropMode::Hard
            } else {
                DropMode::Soft
            };
            println!("🗑️  Suppression [Mode: {:?}]...", mode);
            file_storage::drop_db(&config, &cli.space, &cli.db, mode)?;
            println!("✅ Terminé.");
        }

        // --- GESTION COLLECTIONS ---
        Commands::CreateCollection { name, schema } => {
            let final_uri = if let Some(s) = schema {
                s
            } else {
                println!("🔍 Recherche de '{}' dans l'index système...", name);
                let sys_path = config.db_root(&cli.space, &cli.db).join("_system.json");
                if !sys_path.exists() {
                    return Err(anyhow!("❌ Index _system.json introuvable."));
                }

                let content = fs::read_to_string(&sys_path)?;
                let sys_json: Value = serde_json::from_str(&content)?;
                let ptr = format!("/collections/{}/schema", name);

                if let Some(raw_path) = sys_json.pointer(&ptr).and_then(|v| v.as_str()) {
                    let relative_path = if let Some(idx) = raw_path.find("/schemas/v1/") {
                        &raw_path[idx + "/schemas/v1/".len()..]
                    } else {
                        raw_path
                    };

                    let schema_file_path = config
                        .db_schemas_root(&cli.space, &cli.db)
                        .join("v1")
                        .join(relative_path);
                    if !schema_file_path.exists() {
                        return Err(anyhow!(
                            "❌ INCOHÉRENCE : Schéma introuvable sur disque.\n   Chemin : {:?}",
                            schema_file_path
                        ));
                    }
                    println!("✅ Fichier schéma validé : {:?}", schema_file_path);

                    let abs_uri =
                        format!("db://{}/{}/schemas/v1/{}", cli.space, cli.db, relative_path);
                    println!("🔗 URI Logique résolue : {}", abs_uri);
                    abs_uri
                } else {
                    return Err(anyhow!(
                        "❌ Collection '{}' non trouvée dans _system.json.",
                        name
                    ));
                }
            };

            println!("🚀 Création de '{}'...", name);
            mgr.create_collection(&name, Some(final_uri))?;
            println!("✅ Collection créée.");
        }

        Commands::ListCollections => {
            let cols = mgr.list_collections()?;
            println!("📂 Collections dans {}/{}:", cli.space, cli.db);
            for c in cols {
                println!("  - {}", c);
            }
        }

        Commands::ListAll { collection } => {
            let docs = mgr.list_all(&collection)?;
            println!("--- {} documents ---", docs.len());
            for doc in docs {
                println!("{}", serde_json::to_string(&doc)?);
            }
        }

        Commands::Insert { collection, data } => {
            let content = if data.starts_with('@') {
                fs::read_to_string(&data[1..])?
            } else {
                data
            };
            let doc: Value = serde_json::from_str(&content)?;
            let res = mgr.insert_with_schema(&collection, doc)?;
            println!(
                "✅ Inséré ID: {}",
                res.get("id").and_then(|v| v.as_str()).unwrap_or("?")
            );
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
            let q = genaptitude::json_db::query::sql::parse_sql(&query)?;
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
                    if entry.path().extension().map_or(false, |e| e == "json") {
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
