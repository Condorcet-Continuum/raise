use clap::{Args, Subcommand};

// --- IMPORTS RAISE ---

use raise::json_db::{
    collections::manager::CollectionsManager,
    indexes::manager::IndexManager,
    jsonld::VocabularyRegistry,
    query::{Condition, FilterOperator, Query, QueryEngine, QueryFilter},
    storage::{JsonDbConfig, StorageEngine},
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::{
    user_error, user_info, user_success,
    utils::{
        data::{self, Deserialize, Map, Value},
        io::{self, Path, PathBuf},
        prelude::*,
        Arc, Future, Pin,
    },
};

// --- DÉFINITION DES ARGUMENTS ---

#[derive(Args, Debug, Clone)]
pub struct JsondbArgs {
    #[arg(short, long, default_value = "default_space")]
    pub space: String,

    #[arg(short, long, default_value = "default_db")]
    pub db: String,

    #[arg(long, env = "PATH_RAISE_DOMAIN")]
    pub root: Option<PathBuf>,

    #[command(subcommand)]
    pub command: JsondbCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum JsondbCommands {
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

// --- HANDLER PRINCIPAL ---

pub async fn handle(args: JsondbArgs) -> Result<()> {
    if let JsondbCommands::Usage = args.command {
        print_examples();
        return Ok(());
    }

    // RÉCUPÉRATION DE LA CONFIGURATION
    let app_config = AppConfig::get();
    let root_dir = args.root.unwrap_or_else(|| {
        app_config
            .get_path("PATH_RAISE_DOMAIN")
            .expect("ERREUR: Le chemin PATH_RAISE_DOMAIN est introuvable !")
            .clone()
    });

    bootstrap_ontologies(&root_dir, &args.space).await;

    let config = Arc::new(JsonDbConfig {
        data_root: root_dir.clone(),
    });

    let storage = StorageEngine::new((*config).clone());
    let col_mgr = CollectionsManager::new(&storage, &args.space, &args.db);
    let mut idx_mgr = IndexManager::new(&storage, &args.space, &args.db);
    let tx_mgr = TransactionManager::new(&config, &args.space, &args.db);

    // Feedback contextuel
    if app_config.core.log_level == "debug" || app_config.core.log_level == "trace" {
        user_info!("JSONDB_CTX_ROOT", "{:?}", root_dir);
        user_info!("JSONDB_CTX_SPACE", "{}/{}", args.space, args.db);
    }

    if !matches!(
        args.command,
        JsondbCommands::CreateDb | JsondbCommands::DropDb { .. }
    ) && !config.db_root(&args.space, &args.db).exists()
    {
        user_info!("JSONDB_BOOTSTRAP_AUTO");
        let _ = col_mgr.init_db().await;
    }

    match args.command {
        JsondbCommands::Usage => { /* Géré plus haut */ }
        JsondbCommands::CreateDb => {
            if col_mgr.init_db().await? {
                user_success!("JSONDB_INIT_SUCCESS", "{}/{}", args.space, args.db);
            } else {
                user_info!("JSONDB_EXISTS", "{}/{}", args.space, args.db);
            }
        }
        JsondbCommands::DropDb { force } => {
            if !force {
                user_error!("JSONDB_DROP_WARN");
            } else if col_mgr.drop_db().await? {
                user_success!("JSONDB_DROP_SUCCESS", "{}/{}", args.space, args.db);
            } else {
                user_error!("JSONDB_DROP_NOT_FOUND", "{}/{}", args.space, args.db);
            }
        }
        JsondbCommands::CreateCollection { name, schema } => {
            let raw_schema = schema.ok_or_else(|| {
                AppError::from("⛔ ERREUR : Le paramètre --schema est OBLIGATOIRE.")
            })?;
            let schema_uri = if raw_schema.starts_with("db://") {
                raw_schema
            } else {
                format!("db://{}/{}/schemas/v1/{}", args.space, args.db, raw_schema)
            };
            col_mgr
                .create_collection(&name, Some(schema_uri.clone()))
                .await?;
            user_success!("JSONDB_COL_CREATED", "{}", name);
        }
        JsondbCommands::DropCollection { name } => {
            col_mgr.drop_collection(&name).await?;
            user_success!("JSONDB_COL_DROPPED", "{}", name);
        }
        JsondbCommands::ListCollections => {
            let cols = col_mgr.list_collections().await?;
            // REFAC: Utilisation de data::stringify_pretty
            println!("{}", data::stringify_pretty(&cols)?);
        }
        JsondbCommands::CreateIndex {
            collection,
            field,
            kind,
        } => {
            idx_mgr.create_index(&collection, &field, &kind).await?;
            user_success!("JSONDB_INDEX_CREATED");
        }
        JsondbCommands::DropIndex { collection, field } => {
            idx_mgr.drop_index(&collection, &field).await?;
            user_success!("JSONDB_INDEX_DROPPED");
        }
        JsondbCommands::List { collection } | JsondbCommands::ListAll { collection } => {
            let docs = col_mgr.list_all(&collection).await?;
            // REFAC: Utilisation de data::stringify_pretty
            println!("{}", data::stringify_pretty(&docs)?);
        }
        JsondbCommands::Insert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let resolved_json = resolve_references(json_val, &col_mgr).await?;
            let res = col_mgr
                .insert_with_schema(&collection, resolved_json)
                .await?;

            let id = res.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            user_success!("JSONDB_INSERT_SUCCESS", "{}", id);
        }
        JsondbCommands::Update {
            collection,
            id,
            data,
        } => {
            let json_val = parse_data(&data).await?;
            let resolved_json = resolve_references(json_val, &col_mgr).await?;
            col_mgr
                .update_document(&collection, &id, resolved_json)
                .await?;
            user_success!("JSONDB_UPDATE_SUCCESS", "{}", id);
        }
        JsondbCommands::Upsert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let resolved_json = resolve_references(json_val, &col_mgr).await?;
            let status = col_mgr.upsert_document(&collection, resolved_json).await?;
            user_success!("JSONDB_UPSERT_SUCCESS", "{:?}", status);
        }
        JsondbCommands::Delete { collection, id } => {
            if col_mgr.delete_document(&collection, &id).await? {
                user_success!("JSONDB_DELETE_SUCCESS", "{}", id);
            } else {
                user_error!("JSONDB_DELETE_NOT_FOUND", "{}", id);
            }
        }
        JsondbCommands::Query {
            collection,
            filter,
            limit,
            offset,
        } => {
            let mut query = Query::new(&collection);
            if let Some(f_str) = filter {
                let f_json = parse_data(&f_str).await?;
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
            let result = QueryEngine::new(&col_mgr).execute_query(query).await?;
            // REFAC: Utilisation de data::stringify_pretty
            println!("{}", data::stringify_pretty(&result.documents)?);
        }
        JsondbCommands::Sql { query } => {
            use raise::json_db::query::sql::{parse_sql, SqlRequest};
            match parse_sql(&query)
                .map_err(|e| AppError::from(format!("Erreur de parsing SQL : {}", e)))?
            {
                SqlRequest::Read(query_struct) => {
                    let result = QueryEngine::new(&col_mgr)
                        .execute_query(query_struct)
                        .await?;
                    // REFAC: Utilisation de data::stringify_pretty
                    println!("{}", data::stringify_pretty(&result.documents)?);
                }
                SqlRequest::Write(requests) => {
                    tx_mgr.execute_smart(requests).await?;
                    user_success!("JSONDB_SQL_TX_SUCCESS");
                }
            }
        }
        JsondbCommands::Import { collection, path } => {
            let json: Value = io::read_json(&path).await?;

            let docs = if let Some(arr) = json.as_array() {
                arr.to_vec()
            } else {
                vec![json]
            };
            for doc in docs {
                let resolved = resolve_references(doc, &col_mgr).await?;
                col_mgr.insert_with_schema(&collection, resolved).await?;
            }
            user_success!("JSONDB_IMPORT_SUCCESS");
        }
        JsondbCommands::Transaction { file } => {
            let json_val: Value = io::read_json(&file).await?;

            #[derive(Deserialize)]
            struct Wrapper {
                operations: Vec<TransactionRequest>,
            }

            // REFAC: Utilisation de data::from_value
            let reqs: Vec<TransactionRequest> =
                if let Ok(w) = data::from_value::<Wrapper>(json_val.clone()) {
                    w.operations
                } else {
                    data::from_value::<Vec<TransactionRequest>>(json_val).map_err(|e| {
                        AppError::from(format!("Format de transaction invalide : {}", e))
                    })?
                };

            user_info!("JSONDB_TX_START", "{}", reqs.len());
            tx_mgr.execute_smart(reqs).await?;
            user_success!("JSONDB_TX_SUCCESS");
        }
    }
    Ok(())
}

// --- HELPERS ---

async fn parse_data(input: &str) -> Result<Value> {
    if let Some(path_str) = input.strip_prefix('@') {
        let path = Path::new(path_str);
        let data = io::read_json(path).await?;
        Ok(data)
    } else {
        // REFAC: Utilisation de data::parse
        Ok(data::parse(input)?)
    }
}

async fn bootstrap_ontologies(root_dir: &Path, space: &str) {
    let space_path = root_dir
        .join(space)
        .join("_system/schemas/v1/arcadia/@context");
    let global_path = root_dir.join("ontology/arcadia/@context");
    let target = if space_path.exists() {
        &space_path
    } else {
        &global_path
    };

    if target.exists() {
        let registry = VocabularyRegistry::global();
        for layer in ["oa", "sa", "la", "pa", "epbs", "data", "transverse"] {
            let _ = registry
                .load_layer_from_file(layer, &target.join(format!("{}.jsonld", layer)))
                .await;
        }
    }
}

fn print_examples() {
    user_info!("JSONDB_USAGE_TITLE");
}

fn parse_smart_link(s: &str) -> Option<(&str, &str, &str)> {
    if !s.starts_with("ref:") {
        return None;
    }
    let parts: Vec<&str> = s.splitn(4, ':').collect();
    if parts.len() == 4 {
        Some((parts[1], parts[2], parts[3]))
    } else {
        None
    }
}

fn resolve_references<'a>(
    data: Value,
    col_mgr: &'a CollectionsManager,
) -> Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>> {
    Box::pin(async move {
        match data {
            Value::String(s) => {
                if let Some((col, field, val)) = parse_smart_link(&s) {
                    let mut query = Query::new(col);
                    query.filter = Some(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: vec![Condition::eq(field, val.into())],
                    });
                    let result = QueryEngine::new(col_mgr).execute_query(query).await?;
                    if let Some(doc) = result.documents.first() {
                        let id = doc
                            .get("id")
                            .or_else(|| doc.get("_id"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        Ok(Value::String(id.to_string()))
                    } else {
                        Ok(Value::String(s))
                    }
                } else {
                    Ok(Value::String(s))
                }
            }
            Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for item in arr {
                    new_arr.push(resolve_references(item, col_mgr).await?);
                }
                Ok(Value::Array(new_arr))
            }
            Value::Object(map) => {
                let mut new_map = Map::new();
                for (k, v) in map {
                    new_map.insert(k, resolve_references(v, col_mgr).await?);
                }
                Ok(Value::Object(new_map))
            }
            _ => Ok(data),
        }
    })
}

// --- TESTS UNITAIRES (Patrimoine Conservé & Adapté) ---
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: JsondbArgs,
    }

    #[test]
    fn verify_cli_structure() {
        TestCli::command().debug_assert();
    }

    #[test]
    fn test_parse_create_index_defaults() {
        let args = vec![
            "test",
            "create-index",
            "--collection",
            "users",
            "--field",
            "email",
        ];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::CreateIndex { kind, .. } => assert_eq!(kind, "hash"),
            _ => panic!("Mauvaise commande parsée"),
        }
    }

    #[test]
    fn test_parse_drop_db_flag() {
        let args = vec!["test", "drop-db", "-f"];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::DropDb { force } => assert!(force),
            _ => panic!("Mauvaise commande parsée"),
        }
    }

    #[test]
    fn test_parse_query_optional() {
        let args = vec!["test", "query", "--collection", "users"];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::Query { filter, limit, .. } => {
                assert!(filter.is_none());
                assert!(limit.is_none());
            }
            _ => panic!("Mauvaise commande parsée"),
        }
    }

    #[test]
    fn test_parse_update_command() {
        let args = vec![
            "test",
            "update",
            "--collection",
            "users",
            "--id",
            "123",
            "--data",
            "{}",
        ];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::Update { collection, id, .. } => {
                assert_eq!(collection, "users");
                assert_eq!(id, "123");
            }
            _ => panic!("Parsing update failed"),
        }
    }

    #[test]
    fn test_parse_upsert_command() {
        let args = vec!["test", "upsert", "--collection", "users", "--data", "{}"];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::Upsert { collection, .. } => assert_eq!(collection, "users"),
            _ => panic!("Parsing upsert failed"),
        }
    }

    #[test]
    fn test_parse_delete_command() {
        let args = vec!["test", "delete", "--collection", "items", "--id", "abc"];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::Delete { collection, id } => {
                assert_eq!(collection, "items");
                assert_eq!(id, "abc");
            }
            _ => panic!("Parsing delete failed"),
        }
    }

    #[test]
    fn test_parse_smart_link_valid() {
        let input = "ref:oa_actors:name:Sécurité";
        let res = parse_smart_link(input);
        assert!(res.is_some());
        let (col, field, val) = res.unwrap();
        assert_eq!(col, "oa_actors");
        assert_eq!(field, "name");
        assert_eq!(val, "Sécurité");
    }

    #[test]
    fn test_parse_smart_link_invalid_prefix() {
        assert!(parse_smart_link("uuid:1234-5678").is_none());
    }

    #[test]
    fn test_parse_smart_link_missing_parts() {
        assert!(parse_smart_link("ref:oa_actors:name").is_none());
    }

    #[test]
    fn test_parse_smart_link_complex_value() {
        let input = "ref:oa_actors:description:Ceci:est:une:description";
        let res = parse_smart_link(input);
        assert!(res.is_some());
        let (_col, _field, val) = res.unwrap();
        assert_eq!(val, "Ceci:est:une:description");
    }

    #[test]
    fn test_transaction_wrapper_deserialization() {
        let json = r#"{"operations": []}"#;
        // REFAC: test avec data::parse
        let res: Result<data::Value> = data::parse(json);
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_parse_data_helper_robustness() {
        assert!(parse_data(r#"{"test":true}"#).await.is_ok());
        assert!(parse_data("invalid").await.is_err());
    }
}
