// FICHIER : src-tauri/tools/jsondb_cli/src/main.rs

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use dotenvy::dotenv;
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// --- IMPORTS RAISE ---
use raise::json_db::{
    collections::manager::CollectionsManager,
    indexes::manager::IndexManager,
    jsonld::VocabularyRegistry,
    // Note: 'sql' retiré ici pour éviter le warning "unused import",
    // car on utilise le chemin complet raise::json_db::query::sql::...
    query::QueryEngine,
    storage::{JsonDbConfig, StorageEngine},
    transactions::{manager::TransactionManager, TransactionRequest},
};

// --- DÉFINITION CLI ---

#[derive(Parser, Debug)]
#[command(
    name = "jsondb_cli",
    author = "RAISE Team",
    version,
    about = "Outil d'administration complet pour RAISE JSON-DB"
)]
struct Cli {
    #[arg(short, long, default_value = "default_space")]
    space: String,

    #[arg(short, long, default_value = "default_db")]
    db: String,

    #[arg(long, env = "PATH_RAISE_DOMAIN")]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    // --- AIDE & EXEMPLES ---
    /// Affiche le guide de survie avec exemples
    Usage,

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

    // --- INDEXES ---
    CreateIndex {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        field: String,
        #[arg(long, default_value = "hash")]
        kind: String,
    },
    DropIndex {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        field: String,
    },

    // --- DATA READ ---
    List {
        #[arg(long)]
        collection: String,
    },
    ListAll {
        #[arg(long)]
        collection: String,
    },

    // --- DATA WRITE (CRUD COMPLET) ---
    Insert {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        data: String,
    },
    /// Mise à jour partielle (Merge)
    Update {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        data: String,
    },
    /// Insert ou Update (Idempotent)
    Upsert {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        data: String,
    },
    /// Suppression par ID
    Delete {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        id: String,
    },

    // --- QUERIES & TOOLS ---
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

// --- MAIN ---

#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    if std::env::var("RUST_LOG").is_ok() {
        tracing_subscriber::fmt::init();
    }

    let cli = Cli::parse();

    // 1. GESTION IMMÉDIATE DE L'AIDE
    if let Commands::Usage = cli.command {
        print_examples();
        return Ok(());
    }

    // 2. CONFIGURATION MOTEUR
    let root_dir = cli.root.unwrap_or_else(|| {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("raise_db")
    });

    // --- CHARGEMENT DES ONTOLOGIES (Sémantique) ---
    bootstrap_ontologies(&root_dir, &cli.space);

    let config = Arc::new(JsonDbConfig {
        data_root: root_dir.clone(),
    });

    let storage = StorageEngine::new((*config).clone());
    let col_mgr = CollectionsManager::new(&storage, &cli.space, &cli.db);
    let mut idx_mgr = IndexManager::new(&storage, &cli.space, &cli.db);
    let tx_mgr = TransactionManager::new(&config, &cli.space, &cli.db);

    if std::env::var("RUST_LOG").is_ok() {
        println!("📂 Database Root: {:?}", root_dir);
        println!("🔧 Context: {}/{}", cli.space, cli.db);
    }

    // Auto-bootstrap
    if !matches!(cli.command, Commands::CreateDb | Commands::DropDb { .. })
        && !config.db_root(&cli.space, &cli.db).exists()
    {
        println!("ℹ️  Auto-bootstrap: Initialisation de la base...");
        let _ = col_mgr.init_db().await;
    }

    // 3. EXÉCUTION
    match cli.command {
        Commands::Usage => { /* Déjà géré plus haut */ }

        // --- DB ---
        Commands::CreateDb => {
            col_mgr.init_db().await?;
            println!("✅ Base initialisée.");
        }
        Commands::DropDb { force } => {
            if !force {
                eprintln!("⚠️ Utilisez --force pour confirmer la suppression.");
            } else {
                let db_path = root_dir.join(&cli.space).join(&cli.db);
                if db_path.exists() {
                    fs::remove_dir_all(&db_path)?;
                    println!("🔥 Base supprimée : {:?}", db_path);
                } else {
                    println!("❌ Base introuvable.");
                }
            }
        }

        // --- COLLECTIONS ---
        Commands::CreateCollection { name, schema } => {
            let raw_schema = schema.ok_or_else(|| {
                anyhow::anyhow!("⛔ ERREUR : Le paramètre --schema est OBLIGATOIRE.")
            })?;

            let schema_uri = if raw_schema.starts_with("db://") {
                raw_schema
            } else {
                format!("db://{}/{}/schemas/v1/{}", cli.space, cli.db, raw_schema)
            };

            col_mgr
                .create_collection(&name, Some(schema_uri.clone()))
                .await?;

            println!("✅ Collection '{}' créée.", name);
            println!("   🔗 Schema lié : {}", schema_uri);
        }
        Commands::DropCollection { name } => {
            col_mgr.drop_collection(&name).await?;
            println!("🗑️ Collection '{}' supprimée.", name);
        }
        Commands::ListCollections => {
            let cols = col_mgr.list_collections().await?;
            println!("{}", serde_json::to_string_pretty(&cols)?);
        }

        // --- INDEXES ---
        Commands::CreateIndex {
            collection,
            field,
            kind,
        } => {
            println!("⚙️ Création index {} sur {}.{}", kind, collection, field);
            idx_mgr.create_index(&collection, &field, &kind).await?;
            println!("✅ Index créé.");
        }
        Commands::DropIndex { collection, field } => {
            idx_mgr.drop_index(&collection, &field).await?;
            println!("🗑️ Index supprimé.");
        }

        // --- DATA READ ---
        Commands::List { collection } | Commands::ListAll { collection } => {
            let docs = col_mgr.list_all(&collection).await?;
            println!("{}", serde_json::to_string_pretty(&docs)?);
        }

        // --- DATA WRITE (CRUD) ---
        Commands::Insert { collection, data } => {
            let json_val = parse_data(&data)?;
            let res = col_mgr.insert_with_schema(&collection, json_val).await?;
            let id = res.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            println!("✅ Inséré : {}", id);
        }

        Commands::Update {
            collection,
            id,
            data,
        } => {
            let json_val = parse_data(&data)?;
            let updated = col_mgr.update_document(&collection, &id, json_val).await?;
            println!("✅ Document {} mis à jour (Merge).", id);
            #[cfg(debug_assertions)]
            println!("   -> {}", updated);
        }

        Commands::Upsert { collection, data } => {
            let json_val = parse_data(&data)?;
            let status = col_mgr.upsert_document(&collection, json_val).await?;
            println!("✅ Upsert : {}", status);
        }

        Commands::Delete { collection, id } => {
            let success = col_mgr.delete_document(&collection, &id).await?;
            if success {
                println!("🗑️  Document {} supprimé.", id);
            } else {
                println!("⚠️ Document {} introuvable (ou déjà supprimé).", id);
            }
        }

        // --- QUERIES ---
        Commands::Query {
            collection,
            filter,
            limit,
            offset,
        } => {
            use raise::json_db::query::{Condition, FilterOperator, Query, QueryFilter};

            let mut query = Query::new(&collection);

            if let Some(f_str) = filter {
                let f_json = parse_data(&f_str)?;
                if let Some(obj) = f_json.as_object() {
                    let mut conditions = vec![];
                    for (k, v) in obj {
                        conditions.push(Condition::eq(k, v.clone()));
                    }
                    query.filter = Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions,
                    });
                }
            }

            query.limit = limit;
            query.offset = offset;

            let engine = QueryEngine::new(&col_mgr);
            let result = engine.execute_query(query).await?;

            println!("{}", serde_json::to_string_pretty(&result.documents)?);
        }

        Commands::Sql { query } => {
            use raise::json_db::query::sql::SqlRequest;

            match raise::json_db::query::sql::parse_sql(&query).context("Erreur de parsing SQL")? {
                SqlRequest::Read(query_struct) => {
                    let engine = QueryEngine::new(&col_mgr);
                    let result = engine.execute_query(query_struct).await?;
                    println!("{}", serde_json::to_string_pretty(&result.documents)?);
                }
                SqlRequest::Write(requests) => {
                    println!(
                        "📝 Exécution SQL Transaction ({} opérations)...",
                        requests.len()
                    );
                    tx_mgr.execute_smart(requests).await?;
                    println!("✅ Transaction SQL validée.");
                }
            }
        }

        // --- BATCH ---
        Commands::Import { collection, path } => {
            println!("📦 Import dans '{}' depuis {:?}", collection, path);
            let content = fs::read_to_string(&path)?;
            let json: Value = serde_json::from_str(&content)?;

            let mut count = 0;
            if let Some(arr) = json.as_array() {
                for doc in arr {
                    col_mgr.insert_with_schema(&collection, doc.clone()).await?;
                    count += 1;
                }
            } else {
                col_mgr.insert_with_schema(&collection, json).await?;
                count = 1;
            }
            println!("✅ {} documents importés.", count);
        }

        Commands::Transaction { file } => {
            let content = fs::read_to_string(&file)?;
            #[derive(Deserialize)]
            struct Wrapper {
                operations: Vec<TransactionRequest>,
            }

            let reqs: Vec<TransactionRequest> =
                if let Ok(w) = serde_json::from_str::<Wrapper>(&content) {
                    w.operations
                } else {
                    serde_json::from_str::<Vec<TransactionRequest>>(&content)?
                };

            println!("🔄 Transaction ({} ops)...", reqs.len());
            tx_mgr.execute_smart(reqs).await?;
            println!("✅ Validée.");
        }
    }

    Ok(())
}

fn parse_data(input: &str) -> Result<Value> {
    if let Some(path) = input.strip_prefix('@') {
        let content = fs::read_to_string(path)?;
        Ok(serde_json::from_str(&content)?)
    } else {
        Ok(serde_json::from_str(input)?)
    }
}

// --- FONCTION DE CHARGEMENT DES ONTOLOGIES ---
// Recherche et charge les fichiers JSON-LD dans le registre global
fn bootstrap_ontologies(root_dir: &Path, space: &str) {
    // 1. Chemin spécifique à l'espace
    let space_path = root_dir
        .join(space)
        .join("_system/schemas/v1/arcadia/@context");

    // 2. Chemin global (Fallback)
    let global_path = root_dir.join("ontology/arcadia/@context");

    // CORRECTION E0382 : Utilisation de références (&) pour éviter le déplacement (move)
    let target_path = if space_path.exists() {
        &space_path
    } else {
        &global_path
    };

    if target_path.exists() {
        let registry = VocabularyRegistry::global();
        // Chargement des couches
        let _ = registry.load_layer_from_file("oa", &target_path.join("oa.jsonld"));
        let _ = registry.load_layer_from_file("sa", &target_path.join("sa.jsonld"));
        let _ = registry.load_layer_from_file("la", &target_path.join("la.jsonld"));
        let _ = registry.load_layer_from_file("pa", &target_path.join("pa.jsonld"));
        let _ = registry.load_layer_from_file("epbs", &target_path.join("epbs.jsonld"));
        let _ = registry.load_layer_from_file("data", &target_path.join("data.jsonld"));

        #[cfg(debug_assertions)]
        println!("🧠 Ontologies chargées depuis {:?}", target_path);
    } else {
        #[cfg(debug_assertions)]
        // Ici, on peut utiliser space_path et global_path car ils n'ont pas été "moved"
        println!(
            "⚠️ Dossier ontologie introuvable.\n   Testé : {:?}\n   Et : {:?}",
            space_path, global_path
        );
    }
}

// --- HELPER D'USAGE ---
fn print_examples() {
    println!(
        r#"
🚀 RAISE JSON-DB CLI - Guide de survie
======================================

1️⃣  INITIALISATION
   ./jsondb_cli create-db
   ./jsondb_cli create-collection --name "users" --schema "actors/user.schema.json"

2️⃣  CRUD COMPLET
   ./jsondb_cli insert --collection "users" --data '{{"name": "Alice"}}'
   ./jsondb_cli update --collection "users" --id "UUID" --data '{{"role": "admin"}}'
   ./jsondb_cli upsert --collection "users" --data '{{"id": "fixed", "name": "Bob"}}'
   ./jsondb_cli delete --collection "users" --id "UUID"

3️⃣  SQL & QUERY
   ./jsondb_cli query --collection "users" --filter '{{"age": 30}}'
   ./jsondb_cli sql --query "SELECT name FROM users WHERE age = 30"

4️⃣  TRANSACTIONS & IMPORT
   ./jsondb_cli import --collection "users" --path ./backup.json
   ./jsondb_cli transaction --file tx.json
"#
    );
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
    fn test_parse_create_index_defaults() {
        let args = vec![
            "jsondb_cli",
            "create-index",
            "--collection",
            "users",
            "--field",
            "email",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::CreateIndex { kind, .. } => assert_eq!(kind, "hash"),
            _ => panic!("Mauvaise commande parsée"),
        }
    }

    #[test]
    fn test_parse_drop_db_flag() {
        let args = vec!["jsondb_cli", "drop-db", "-f"];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::DropDb { force } => assert!(force),
            _ => panic!("Mauvaise commande parsée"),
        }
    }

    #[test]
    fn test_parse_query_optional() {
        let args = vec!["jsondb_cli", "query", "--collection", "users"];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Query { filter, limit, .. } => {
                assert!(filter.is_none());
                assert!(limit.is_none());
            }
            _ => panic!("Mauvaise commande parsée"),
        }
    }

    #[test]
    fn test_parse_update_command() {
        let args = vec![
            "jsondb_cli",
            "update",
            "--collection",
            "users",
            "--id",
            "123",
            "--data",
            "{}",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Update { collection, id, .. } => {
                assert_eq!(collection, "users");
                assert_eq!(id, "123");
            }
            _ => panic!("Parsing update failed"),
        }
    }

    #[test]
    fn test_parse_upsert_command() {
        let args = vec![
            "jsondb_cli",
            "upsert",
            "--collection",
            "users",
            "--data",
            "{}",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Upsert { collection, .. } => {
                assert_eq!(collection, "users");
            }
            _ => panic!("Parsing upsert failed"),
        }
    }

    #[test]
    fn test_parse_delete_command() {
        let args = vec![
            "jsondb_cli",
            "delete",
            "--collection",
            "items",
            "--id",
            "abc",
        ];
        let cli = Cli::parse_from(args);
        match cli.command {
            Commands::Delete { collection, id } => {
                assert_eq!(collection, "items");
                assert_eq!(id, "abc");
            }
            _ => panic!("Parsing delete failed"),
        }
    }
}
