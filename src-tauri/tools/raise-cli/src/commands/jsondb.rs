// FICHIER : src-tauri/tools/raise-cli/src/commands/jsondb.rs

use clap::{Args, Subcommand};

// --- IMPORTS RAISE ---
use raise::json_db::{
    collections::manager::{CollectionsManager, EntityIdentity},
    indexes::manager::IndexManager,
    jsonld::VocabularyRegistry,
    query::{Condition, FilterOperator, Projection, Query, QueryEngine, QueryFilter},
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::{
    raise_error, user_error, user_info, user_success,
    utils::{
        data::{self, Deserialize, Value},
        io::{self, Path, PathBuf},
        prelude::*,
    },
};

// 🎯 NOUVEAU : Import du contexte global CLI
use crate::CliContext;

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

    // --- GESTION DES SCHÉMAS (DDL) ---
    ListSchemas,
    CreateSchema {
        #[arg(long)]
        name: String,
        #[arg(long)]
        schema: String,
    },
    DropSchema {
        #[arg(long)]
        name: String,
    },
    AddSchemaProperty {
        #[arg(long)]
        name: String,
        #[arg(long)]
        property: String,
        #[arg(long)]
        definition: String,
    },
    AlterSchemaProperty {
        #[arg(long)]
        name: String,
        #[arg(long)]
        property: String,
        #[arg(long)]
        definition: String,
    },
    DropSchemaProperty {
        #[arg(long)]
        name: String,
        #[arg(long)]
        property: String,
    },

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
    ListIndexes {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        field: Option<String>,
    },
    // --- DATA READ ---
    List {
        #[arg(long)]
        collection: String,
        #[arg(long, short = 'f', value_delimiter = ' ', num_args = 1..)]
        fields: Option<Vec<String>>,
    },
    ListAll {
        #[arg(long)]
        collection: String,
        #[arg(long, short = 'f', value_delimiter = ' ', num_args = 1..)]
        fields: Option<Vec<String>>,
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
    /// Suppression par ID ou Name
    Delete {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        name: Option<String>,
    },

    // --- QUERIES & TOOLS ---
    Query {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        filter: Option<String>,
        #[arg(long, short = 'f', value_delimiter = ' ', num_args = 1..)]
        fields: Option<Vec<String>>,
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

// 🎯 La signature intègre le CliContext
pub async fn handle(args: JsondbArgs, ctx: CliContext) -> RaiseResult<()> {
    if let JsondbCommands::Usage = args.command {
        print_examples();
        return Ok(());
    }

    // 🎯 Heartbeat automatique
    let _ = ctx.session_mgr.touch().await;

    // 🎯 On utilise le storage du contexte au lieu de recréer
    let storage = &ctx.storage;
    let root_dir = storage.config.data_root.clone();

    bootstrap_ontologies(&root_dir, &args.space).await;

    let col_mgr = CollectionsManager::new(storage, &args.space, &args.db);
    let mut idx_mgr = IndexManager::new(storage, &args.space, &args.db);
    let tx_mgr = TransactionManager::new(storage, &args.space, &args.db);

    // Feedback contextuel
    if ctx.config.core.log_level == "debug" || ctx.config.core.log_level == "trace" {
        user_info!("JSONDB_CTX_ROOT", json!({ "path": root_dir }));
        user_info!(
            "JSONDB_CTX_SPACE",
            json!({
                "space": args.space,
                "db": args.db,
                "action": "load_context"
            })
        );
    }

    if !matches!(
        args.command,
        JsondbCommands::CreateDb | JsondbCommands::DropDb { .. }
    ) && !storage.config.db_root(&args.space, &args.db).exists()
    {
        user_info!("JSONDB_BOOTSTRAP_AUTO", json!({"status": "starting"}));
        let _ = col_mgr.init_db().await;
    }

    match args.command {
        JsondbCommands::Usage => { /* Géré plus haut */ }

        // --- GESTION DES SCHÉMAS (DDL) ---
        JsondbCommands::ListSchemas => {
            let schemas = col_mgr.list_schemas().await?;
            user_success!(
                "JSONDB_SCHEMAS_LISTED",
                json!({ "space": args.space, "db": args.db, "count": schemas.len() })
            );
            println!("{}", data::stringify_pretty(&schemas)?);
        }
        JsondbCommands::CreateSchema { name, schema } => {
            let schema_val = parse_data(&schema).await?;
            col_mgr.create_schema_def(&name, schema_val).await?;
            user_success!(
                "JSONDB_SCHEMA_CREATED",
                json!({ "schema": name, "status": "created" })
            );
        }
        JsondbCommands::DropSchema { name } => {
            col_mgr.drop_schema_def(&name).await?;
            user_success!(
                "JSONDB_SCHEMA_DROPPED",
                json!({ "schema": name, "status": "dropped" })
            );
        }
        JsondbCommands::AddSchemaProperty {
            name,
            property,
            definition,
        } => {
            let def_val = parse_data(&definition).await?;
            col_mgr
                .add_schema_property(&name, &property, def_val)
                .await?;
            user_success!(
                "JSONDB_SCHEMA_PROP_ADDED",
                json!({ "schema": name, "property": property, "status": "added" })
            );
        }
        JsondbCommands::AlterSchemaProperty {
            name,
            property,
            definition,
        } => {
            let def_val = parse_data(&definition).await?;
            col_mgr
                .alter_schema_property(&name, &property, def_val)
                .await?;
            user_success!(
                "JSONDB_SCHEMA_PROP_ALTERED",
                json!({ "schema": name, "property": property, "status": "altered" })
            );
        }
        JsondbCommands::DropSchemaProperty { name, property } => {
            col_mgr.drop_schema_property(&name, &property).await?;
            user_success!(
                "JSONDB_SCHEMA_PROP_DROPPED",
                json!({ "schema": name, "property": property, "status": "dropped" })
            );
        }

        JsondbCommands::CreateDb => {
            if col_mgr.init_db().await? {
                user_success!(
                    "JSONDB_INIT_SUCCESS",
                    json!({ "space": args.space, "db": args.db })
                );
            } else {
                user_info!(
                    "JSONDB_EXISTS",
                    json!({ "space": args.space, "db": args.db })
                );
            }
        }
        JsondbCommands::DropDb { force } => {
            if !force {
                // 🎯 Utilisation stricte avec contexte JSON vide
                user_error!("JSONDB_DROP_WARN", json!({}));
            } else if col_mgr.drop_db().await? {
                user_success!(
                    "JSONDB_DROP_SUCCESS",
                    json!({ "space": args.space, "db": args.db, "action": "permanent_deletion" })
                );
            } else {
                user_error!(
                    "JSONDB_DROP_NOT_FOUND",
                    json!({ "space": args.space, "db": args.db })
                );
            }
        }
        JsondbCommands::CreateCollection { name, schema } => {
            let Some(raw_schema) = schema else {
                raise_error!(
                    "ERR_CLI_MISSING_SCHEMA",
                    error = "REQUIRED_ARG_NOT_FOUND",
                    context = json!({
                        "action": "validate_command_args",
                        "param": "--schema",
                        "hint": "L'argument --schema est nécessaire pour initialiser la structure. Exemple: --schema user.json"
                    })
                );
            };
            let schema_uri = if raw_schema.starts_with("db://") {
                raw_schema
            } else {
                format!("db://{}/{}/schemas/v1/{}", args.space, args.db, raw_schema)
            };
            col_mgr.create_collection(&name, &schema_uri).await?;
            user_success!(
                "JSONDB_COL_CREATED",
                json!({ "collection": name, "status": "active" })
            );
        }
        JsondbCommands::DropCollection { name } => {
            col_mgr.drop_collection(&name).await?;
            user_success!(
                "JSONDB_COL_DROPPED",
                json!({ "collection": name, "action": "cleanup" })
            );
        }
        JsondbCommands::ListCollections => {
            let cols = col_mgr.list_collections().await?;
            println!("{}", data::stringify_pretty(&cols)?);
        }
        JsondbCommands::CreateIndex {
            collection,
            field,
            kind,
        } => {
            idx_mgr.create_index(&collection, &field, &kind).await?;
            user_success!(
                "JSONDB_INDEX_CREATED",
                json!({ "collection": collection, "field": field, "type": kind })
            );
        }
        JsondbCommands::DropIndex { collection, field } => {
            idx_mgr.drop_index(&collection, &field).await?;
            // 🎯 Ajout d'un contexte JSON
            user_success!(
                "JSONDB_INDEX_DROPPED",
                json!({"collection": collection, "field": field})
            );
        }
        JsondbCommands::ListIndexes { collection, field } => {
            let indexes = idx_mgr.list_indexes(&collection, field.as_deref()).await?;
            println!("{}", data::stringify_pretty(&indexes)?);
        }
        JsondbCommands::List { collection, fields }
        | JsondbCommands::ListAll { collection, fields } => {
            let mut query = Query::new(&collection);
            if let Some(f) = fields {
                query.projection = Some(Projection::Include(f));
            }
            let result = QueryEngine::new(&col_mgr).execute_query(query).await?;
            user_success!(
                "JSONDB_LIST_SUCCESS",
                json!({
                    "collection": collection,
                    "count": result.total_count
                })
            );
            println!("{}", data::stringify_pretty(&result.documents)?);
        }
        JsondbCommands::Insert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let res = col_mgr.insert_with_schema(&collection, json_val).await?;

            let id = res.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            user_success!(
                "JSONDB_INSERT_SUCCESS",
                json!({ "id": id, "status": "persisted" })
            );
        }
        JsondbCommands::Update {
            collection,
            id,
            data,
        } => {
            let json_val = parse_data(&data).await?;
            col_mgr.update_document(&collection, &id, json_val).await?;
            user_success!(
                "JSONDB_UPDATE_SUCCESS",
                json!({ "id": id, "action": "update" })
            );
        }
        JsondbCommands::Upsert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let status = col_mgr.upsert_document(&collection, json_val).await?;
            user_success!(
                "JSONDB_UPSERT_SUCCESS",
                json!({ "status": format!("{:?}", status) })
            );
        }
        JsondbCommands::Delete {
            collection,
            id,
            name,
        } => {
            let identity = if let Some(id_val) = id {
                EntityIdentity::Id(id_val.clone())
            } else if let Some(name_val) = name {
                EntityIdentity::Name(name_val.clone())
            } else {
                raise_error!("ERR_CLI_MISSING_ARG", error = "Fournir --id ou --name");
            };

            col_mgr.delete_identity(&collection, identity).await?;

            user_success!(
                "JSONDB_DELETE_SUCCESS",
                json!({ "collection": collection, "status": "deleted" })
            );
        }
        JsondbCommands::Query {
            collection,
            filter,
            fields,
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
            if let Some(f) = fields {
                query.projection = Some(Projection::Include(f));
            }
            let result = QueryEngine::new(&col_mgr).execute_query(query).await?;
            println!("{}", data::stringify_pretty(&result.documents)?);
        }
        JsondbCommands::Sql { query } => {
            use raise::json_db::query::sql::{parse_sql, SqlRequest};
            let sql_request = match parse_sql(&query) {
                Ok(req) => req,
                Err(e) => raise_error!(
                    "ERR_SQL_PARSE_FAILURE",
                    error = e,
                    context = json!({
                        "action": "parse_sql_query",
                        "query_preview": query.chars().take(100).collect::<String>(),
                        "hint": "Vérifiez la syntaxe SQL (mots-clés, virgules ou guillemets manquants)."
                    })
                ),
            };

            match sql_request {
                SqlRequest::Read(query_struct) => {
                    let result = QueryEngine::new(&col_mgr)
                        .execute_query(query_struct)
                        .await?;

                    println!("{}", data::stringify_pretty(&result.documents)?);
                }
                SqlRequest::Write(requests) => {
                    tx_mgr.execute_smart(requests).await?;
                    user_success!("JSONDB_SQL_TX_SUCCESS", json!({"status": "committed"}));
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

            // 🎯 CORRECTIF : On sauvegarde le compte avant de consommer (move) le vecteur
            let docs_count = docs.len();

            for doc in docs {
                col_mgr.insert_with_schema(&collection, doc).await?;
            }

            // 🎯 On utilise docs_count au lieu de docs.len()
            user_success!(
                "JSONDB_IMPORT_SUCCESS",
                json!({"collection": collection, "docs_imported": docs_count})
            );
        }
        JsondbCommands::Transaction { file } => {
            let json_val: Value = io::read_json(&file).await?;

            #[derive(Deserialize)]
            struct Wrapper {
                operations: Vec<TransactionRequest>,
            }

            let reqs: Vec<TransactionRequest> = if let Ok(w) =
                data::from_value::<Wrapper>(json_val.clone())
            {
                w.operations
            } else {
                match data::from_value::<Vec<TransactionRequest>>(json_val) {
                    Ok(ops) => ops,
                    Err(e) => raise_error!(
                        "ERR_JSONDB_INVALID_FORMAT",
                        error = e,
                        context = json!({
                            "action": "parse_transaction_fallback",
                            "attempted_types": ["Wrapper", "Vec<TransactionRequest>"],
                            "hint": "Le JSON fourni ne correspond à aucun des schémas de transaction supportés."
                        })
                    ),
                }
            };

            user_info!(
                "JSONDB_TX_START",
                json!({ "batch_size": reqs.len(), "mode": "atomic" })
            );
            tx_mgr.execute_smart(reqs).await?;
            // 🎯 Ajout d'un contexte JSON
            user_success!("JSONDB_TX_SUCCESS", json!({"status": "committed"}));
        }
    }
    Ok(())
}

// --- HELPERS ---

async fn parse_data(input: &str) -> RaiseResult<Value> {
    if let Some(path_str) = input.strip_prefix('@') {
        let path = Path::new(path_str);
        let data = io::read_json(path).await?;
        Ok(data)
    } else {
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
    user_info!("JSONDB_USAGE_TITLE", json!({}));
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
    fn test_parse_list_indexes_command() {
        let args = vec![
            "test",
            "list-indexes",
            "--collection",
            "users",
            "--field",
            "email",
        ];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::ListIndexes { collection, field } => {
                assert_eq!(collection, "users");
                assert_eq!(field, Some("email".to_string()));
            }
            _ => panic!("Parsing list-indexes failed"),
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
            JsondbCommands::Delete {
                collection,
                id,
                name: _,
            } => {
                assert_eq!(collection, "items");
                assert_eq!(id, Some("abc".to_string()));
            }
            _ => panic!("Parsing delete failed"),
        }
    }

    #[test]
    fn test_transaction_wrapper_deserialization() {
        let json = r#"{"operations": []}"#;
        let res: RaiseResult<data::Value> = data::parse(json);
        assert!(res.is_ok());
    }

    #[tokio::test]
    async fn test_parse_data_helper_robustness() {
        assert!(parse_data(r#"{"test":true}"#).await.is_ok());
        assert!(parse_data("invalid").await.is_err());
    }

    #[test]
    fn test_parse_create_schema_command() {
        let args = vec![
            "test",
            "create-schema",
            "--name",
            "db://test/schema",
            "--schema",
            "{}",
        ];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::CreateSchema { name, .. } => {
                assert_eq!(name, "db://test/schema");
            }
            _ => panic!("Parsing create-schema failed"),
        }
    }
}
