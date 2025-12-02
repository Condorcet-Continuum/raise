// FICHIER : src-tauri/tools/jsondb_cli/src/main.rs

use anyhow::Result;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

// Imports depuis la librairie core 'genaptitude'
use genaptitude::json_db::collections::manager::CollectionsManager;
use genaptitude::json_db::query::{Query, QueryEngine};
use genaptitude::json_db::storage::{file_storage, JsonDbConfig, StorageEngine};
use genaptitude::json_db::transactions::manager::TransactionManager;
use genaptitude::json_db::transactions::Operation;

#[derive(Parser)]
#[command(
    name = "jsondb_cli",
    author = "GenAptitude Team",
    version,
    about = "Outil d'administration pour GenAptitude JSON-DB",
    long_about = r#"
🚀 GENAPTITUDE JSON-DB CLI

Outil en ligne de commande pour administrer, interroger et debugger la base de données 
JSON locale de GenAptitude sans avoir à lancer l'interface graphique complète.

FONCTIONNALITÉS :
  - Gestion des collections et schémas
  - CRUD (Create, Read, Update, Delete) sur les documents
  - Moteur de requête SQL
  - Import/Export de données
  - Exécution de transactions atomiques

VARIABLES D'ENVIRONNEMENT :
  JSONDB_DATA_ROOT : Chemin vers le dossier de stockage (défaut: ./data)
"#
)]
struct Cli {
    /// Espace de noms (tenant)
    #[arg(
        short,
        long,
        default_value = "default_space",
        help = "L'espace logique (ex: 'un2', 'system')"
    )]
    space: String,

    /// Nom de la base de données
    #[arg(
        short,
        long,
        default_value = "default_db",
        help = "Le nom de la base (ex: '_system')"
    )]
    db: String,

    /// Chemin racine du stockage
    #[arg(
        long,
        env = "JSONDB_DATA_ROOT",
        default_value = "./data",
        help = "Dossier racine contenant les fichiers JSON"
    )]
    root: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Crée une nouvelle collection
    #[command(
        long_about = "Crée un dossier pour la collection et initialise son fichier de métadonnées."
    )]
    CreateCollection {
        /// Nom de la collection (ex: 'actors')
        name: String,

        /// URI du schéma optionnel (ex: 'db://space/db/schemas/v1/actor.json')
        #[arg(long)]
        schema: Option<String>,
    },

    /// Liste toutes les collections existantes
    ListCollections,

    /// Affiche tous les documents d'une collection
    #[command(
        long_about = "Récupère et affiche tous les documents JSON d'une collection.\nAttention : Peut être lent sur de très grosses collections."
    )]
    ListAll {
        /// Nom de la collection cible
        collection: String,
    },

    /// Insère un document JSON
    #[command(long_about = r#"
Insère un document dans la collection.
Gère automatiquement la génération d'ID (UUID v4) et les champs calculés (x_compute) si un schéma est lié.

MODES D'ENTRÉE :
  1. JSON Direct : --data '{"name": "Test"}'
  2. Fichier :     --data '@./mon_fichier.json'
"#)]
    Insert {
        /// Nom de la collection cible
        collection: String,

        /// Contenu JSON ou chemin de fichier (préfixé par @)
        data: String,
    },

    /// Exécute une requête structurée (Filtres JSON)
    #[command(
        long_about = "Effectue une recherche via le moteur de requête interne (QueryEngine).\nSupporte la pagination."
    )]
    Query {
        /// Collection à interroger
        collection: String,

        /// Filtre JSON (Non implémenté complètement en CLI, placeholder)
        #[arg(long)]
        filter: Option<String>,

        /// Nombre maximum de résultats
        #[arg(long)]
        limit: Option<usize>,

        /// Nombre de résultats à sauter
        #[arg(long)]
        offset: Option<usize>,
    },

    /// Exécute une requête SQL
    #[command(long_about = r#"
Exécute une requête SQL standard sur les fichiers JSON.

EXEMPLES :
  jsondb_cli sql --query "SELECT * FROM actors WHERE kind = 'human' ORDER BY age DESC"
  jsondb_cli sql --query "SELECT handle, displayName FROM users"

LIMITATIONS ACTUELLES :
  - Clauses LIMIT et OFFSET ignorées (filtrage et tri ok)
  - Pas de JOIN
"#)]
    Sql {
        /// La chaîne SQL à exécuter
        query: String,
    },

    /// Importe des fichiers JSON en masse
    #[command(
        long_about = "Importe un fichier unique ou tout un dossier de fichiers .json dans une collection."
    )]
    Import {
        /// Collection de destination
        collection: String,

        /// Chemin du fichier ou du dossier à importer
        path: PathBuf,
    },

    /// Exécute une transaction atomique depuis un fichier
    #[command(long_about = r#"
Lit un fichier JSON contenant un tableau d'opérations et les exécute de manière atomique.

FORMAT DU FICHIER :
[
  { "type": "insert", "collection": "logs", "id": "1", "document": {...} },
  { "type": "update", "collection": "users", "id": "u1", "document": {...} },
  { "type": "delete", "collection": "temp", "id": "t1" }
]
"#)]
    Transaction {
        /// Chemin vers le fichier de transaction (.json)
        file: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // 1. Configuration du stockage
    let config = JsonDbConfig {
        data_root: cli.root.clone(),
    };

    // Assure l'existence de la DB physique
    if !config.db_root(&cli.space, &cli.db).exists() {
        println!(
            "ℹ️  Initialisation de la base de données à : {:?}",
            config.db_root(&cli.space, &cli.db)
        );
        file_storage::create_db(&config, &cli.space, &cli.db)?;
    }

    let storage = StorageEngine::new(config.clone());
    let mgr = CollectionsManager::new(&storage, &cli.space, &cli.db);

    match cli.command {
        Commands::CreateCollection { name, schema } => {
            mgr.create_collection(&name, schema.clone())?;
            println!("✅ Collection '{}' créée.", name);
            if let Some(s) = schema {
                println!("   Lien Schéma : {}", s);
            }
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
            println!("--- {} documents dans '{}' ---", docs.len(), collection);
            for doc in docs {
                println!("{}", serde_json::to_string(&doc)?);
            }
        }
        Commands::Insert { collection, data } => {
            let content = if data.starts_with('@') {
                let path = &data[1..];
                fs::read_to_string(path)
                    .map_err(|e| anyhow::anyhow!("Impossible de lire le fichier {}: {}", path, e))?
            } else {
                data
            };

            let doc: Value = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("JSON invalide : {}", e))?;

            // Utilise la méthode intelligente qui gère les schémas et IDs
            let res = mgr.insert_with_schema(&collection, doc)?;
            println!("✅ Document inséré avec succès.");
            println!(
                "   ID : {}",
                res.get("id").and_then(|v| v.as_str()).unwrap_or("?")
            );
        }
        Commands::Query {
            collection,
            filter,
            limit,
            offset,
        } => {
            let filter_obj = if let Some(f) = filter {
                println!("⚠️  Note: Le parsing de filtre complexe depuis CLI n'est pas complet.");
                println!("    Reçu: {}", f);
                None
            } else {
                None
            };

            let query = Query {
                collection: collection.clone(),
                filter: filter_obj,
                sort: None,
                limit,
                offset,
                projection: None,
            };

            let engine = QueryEngine::new(&mgr);
            let result = engine.execute_query(query).await?;

            println!(
                "🔎 Résultat : {} documents (Total estimé: {})",
                result.documents.len(),
                result.total_count
            );
            for doc in result.documents {
                println!("{}", doc);
            }
        }
        Commands::Sql { query } => {
            // Utilise le parser SQL intégré à la librairie
            let q = genaptitude::json_db::query::sql::parse_sql(&query)?;
            let engine = QueryEngine::new(&mgr);
            let result = engine.execute_query(q).await?;

            println!(
                "⚡ SQL Result : {} documents trouvés",
                result.documents.len()
            );
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
                let doc: Value = serde_json::from_str(&content)?;
                mgr.insert_with_schema(&collection, doc)?;
                count += 1;
            }
            println!(
                "\n📦 Import terminé : {} documents ajoutés à '{}'.",
                count, collection
            );
        }
        Commands::Transaction { file } => {
            let content = fs::read_to_string(&file).map_err(|e| {
                anyhow::anyhow!("Impossible de lire le fichier de transaction : {}", e)
            })?;
            let ops: Vec<TxOp> = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Format de transaction invalide : {}", e))?;

            let tm = TransactionManager::new(&config, &cli.space, &cli.db);

            tm.execute(|tx| {
                for op in ops {
                    match op {
                        TxOp::Insert {
                            collection,
                            id,
                            document,
                        } => {
                            tx.operations.push(Operation::Insert {
                                collection,
                                id,
                                document,
                            });
                        }
                        TxOp::Update {
                            collection,
                            id,
                            document,
                        } => {
                            tx.operations.push(Operation::Update {
                                collection,
                                id,
                                document,
                            });
                        }
                        TxOp::Delete { collection, id } => {
                            tx.operations.push(Operation::Delete { collection, id });
                        }
                    }
                }
                Ok(())
            })?;
            println!("🔄 Transaction exécutée avec succès (WAL commit).");
        }
    }

    Ok(())
}

// Structure locale pour désérialiser le fichier de transaction JSON
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum TxOp {
    Insert {
        collection: String,
        id: String,
        document: Value,
    },
    Update {
        collection: String,
        id: String,
        document: Value,
    },
    Delete {
        collection: String,
        id: String,
    },
}
