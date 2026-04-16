// FICHIER : src-tauri/tools/raise-cli/src/commands/jsondb.rs

use clap::{Args, Subcommand};

// --- IMPORTS RAISE ---
use raise::json_db::{
    collections::manager::CollectionsManager,
    indexes::manager::IndexManager,
    query::{Condition, FilterOperator, Projection, Query, QueryEngine, QueryFilter},
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE

// 🎯 Import du contexte global CLI
use crate::CliContext;

#[derive(Args, Debug, Clone)]
pub struct JsondbArgs {
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

    /// Enregistre une ontologie (JSON-LD) dans le registre sémantique
    RegisterOntology {
        #[arg(long, help = "Espace de nom (ex: arcadia, raise)")]
        namespace: String,
        #[arg(long, help = "URI du fichier maître JSON-LD")]
        uri: String,
        #[arg(long, help = "Version sémantique exigeée")]
        version: String,
    },

    // --- DB & COLLECTIONS ---
    CreateDb {
        #[arg(long, required = true, help = "URI du schéma d'index obligatoire")]
        schema: String,
        #[arg(
            long,
            required = true,
            help = "Rôle architectural (system, simulation...)"
        )]
        db_role: String,
    },
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

    // --- DATA ---
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
    Insert {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        data: String,
    },
    Update {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        id: String,
        #[arg(long)]
        data: String,
    },
    Upsert {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        data: String,
    },
    Delete {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        id: Option<String>,
        #[arg(long)]
        handle: Option<String>,
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
    ImportSchemas {
        #[arg(long)]
        source_domain: String,
        #[arg(long)]
        source_db: String,
    },
    Transaction {
        #[arg(long)]
        file: PathBuf,
    },
}

pub async fn handle(args: JsondbArgs, ctx: CliContext) -> RaiseResult<()> {
    if let JsondbCommands::Usage = args.command {
        user_info!("JSONDB_USAGE_TITLE", json_value!({}));
        return Ok(());
    }

    let _ = ctx.session_mgr.touch().await;
    let storage = &ctx.storage;
    let active_domain = &ctx.active_domain;
    let active_db = &ctx.active_db;

    let col_mgr = CollectionsManager::new(storage, active_domain, active_db);
    let mut idx_mgr = IndexManager::new(storage, active_domain, active_db);
    let tx_mgr = TransactionManager::new(storage, active_domain, active_db);

    // Vérification de l'existence de la base (sauf pour création)
    if !matches!(args.command, JsondbCommands::CreateDb { .. })
        && !storage.config.db_root(active_domain, active_db).exists()
    {
        raise_error!(
            "ERR_DB_NOT_FOUND",
            error = format!("La base de données '{active_domain}/{active_db}' n'existe pas."),
            context = json_value!({ "hint": "Initialisez la base avec 'create-db'." })
        );
    }

    match args.command {
        JsondbCommands::Usage => (),
        JsondbCommands::ListSchemas => {
            let schemas = col_mgr.list_schemas().await?;
            println!("{}", json::serialize_to_string_pretty(&schemas)?);
        }
        JsondbCommands::CreateSchema { name, schema } => {
            let schema_val = parse_data(&schema).await?;
            col_mgr.create_schema_def(&name, schema_val).await?;
            user_success!("JSONDB_SCHEMA_CREATED", json_value!({ "schema": name }));
        }
        JsondbCommands::DropSchema { name } => {
            col_mgr.drop_schema_def(&name).await?;
            user_success!("JSONDB_SCHEMA_DROPPED", json_value!({ "schema": name }));
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
                json_value!({ "property": property })
            );
        }
        JsondbCommands::RegisterOntology {
            namespace,
            uri,
            version,
        } => {
            col_mgr
                .register_ontology(&namespace, &uri, &version)
                .await?;
            user_success!(
                "JSONDB_ONTOLOGY_REGISTERED",
                json_value!({ "namespace": namespace })
            );
        }
        JsondbCommands::CreateDb { schema, db_role } => {
            let created = col_mgr.create_db_with_schema(&schema).await?;
            if created {
                // Enrichissement du rôle métier
                let sys_path = storage
                    .config
                    .db_root(active_domain, active_db)
                    .join("_system.json");
                if let Ok(mut sys_json) = fs::read_json_async::<JsonValue>(&sys_path).await {
                    if let Some(obj) = sys_json.as_object_mut() {
                        obj.insert("db_role".to_string(), json_value!(db_role));
                    }
                    let _ = fs::write_json_atomic_async(&sys_path, &sys_json).await;
                }
                user_success!(
                    "JSONDB_INIT_SUCCESS",
                    json_value!({ "space": active_domain })
                );
            }
        }
        JsondbCommands::DropDb { force } => {
            if !force {
                user_error!(
                    "JSONDB_DROP_WARN",
                    json_value!({ "hint": "Utilisez -f pour confirmer." })
                );
            } else {
                col_mgr.drop_db().await?;
                user_success!(
                    "JSONDB_DROP_SUCCESS",
                    json_value!({ "space": active_domain })
                );
            }
        }
        JsondbCommands::CreateCollection { name, schema } => {
            let Some(raw_schema) = schema else {
                raise_error!(
                    "ERR_CLI_MISSING_SCHEMA",
                    error = "Paramètre --schema manquant."
                );
            };
            let schema_uri = if raw_schema.starts_with("db://") {
                raw_schema
            } else {
                format!(
                    "db://{}/{}/schemas/v1/{}",
                    active_domain, active_db, raw_schema
                )
            };
            col_mgr.create_collection(&name, &schema_uri).await?;
            user_success!("JSONDB_COL_CREATED", json_value!({ "collection": name }));
        }
        JsondbCommands::DropCollection { name } => {
            col_mgr.drop_collection(&name).await?;
            user_success!("JSONDB_COL_DROPPED", json_value!({ "collection": name }));
        }
        JsondbCommands::ListCollections => {
            let cols = col_mgr.list_collections().await?;
            println!("{}", json::serialize_to_string_pretty(&cols)?);
        }
        JsondbCommands::CreateIndex {
            collection,
            field,
            kind,
        } => {
            idx_mgr.create_index(&collection, &field, &kind).await?;
            user_success!("JSONDB_INDEX_CREATED", json_value!({ "field": field }));
        }
        JsondbCommands::DropIndex { collection, field } => {
            idx_mgr.drop_index(&collection, &field).await?;
            user_success!("JSONDB_INDEX_DROPPED", json_value!({ "field": field }));
        }
        JsondbCommands::ListIndexes { collection, field } => {
            let indexes = idx_mgr.list_indexes(&collection, field.as_deref()).await?;
            println!("{}", json::serialize_to_string_pretty(&indexes)?);
        }
        JsondbCommands::List { collection, fields }
        | JsondbCommands::ListAll { collection, fields } => {
            let mut query = Query::new(&collection);
            if let Some(f) = fields {
                query.projection = Some(Projection::Include(f));
            }
            let result = QueryEngine::new(&col_mgr).execute_query(query).await?;
            println!("{}", json::serialize_to_string_pretty(&result.documents)?);
        }
        JsondbCommands::Insert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let res = col_mgr.insert_with_schema(&collection, json_val).await?;
            user_success!("JSONDB_INSERT_SUCCESS", json_value!({ "id": res["_id"] }));
        }
        JsondbCommands::Update {
            collection,
            id,
            data,
        } => {
            let json_val = parse_data(&data).await?;
            col_mgr.update_document(&collection, &id, json_val).await?;
            user_success!("JSONDB_UPDATE_SUCCESS", json_value!({ "id": id }));
        }
        JsondbCommands::Upsert { collection, data } => {
            let json_val = parse_data(&data).await?;
            col_mgr.upsert_document(&collection, json_val).await?;
            user_success!("JSONDB_UPSERT_SUCCESS", json_value!({}));
        }
        JsondbCommands::Delete {
            collection,
            id,
            handle,
        } => {
            let target = id.or(handle).ok_or_else(|| {
                build_error!("ERR_CLI_MISSING_ARG", error = "Fournir --id ou --handle")
            })?;
            let doc = col_mgr
                .get_document(&collection, &target)
                .await?
                .ok_or_else(|| {
                    build_error!("ERR_DB_ENTITY_NOT_FOUND", error = "Document introuvable")
                })?;
            let doc_id = doc["_id"]
                .as_str()
                .ok_or_else(|| build_error!("ERR_DB_CORRUPTION", error = "Document sans _id"))?;
            col_mgr.delete_document(&collection, doc_id).await?;
            user_success!(
                "JSONDB_DELETE_SUCCESS",
                json_value!({ "collection": collection })
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
                    let conditions = obj
                        .iter()
                        .map(|(k, v)| Condition::eq(k, v.clone()))
                        .collect();
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
            println!("{}", json::serialize_to_string_pretty(&result.documents)?);
        }
        JsondbCommands::Sql { query } => {
            use raise::json_db::query::sql::{parse_sql, SqlRequest};
            let request =
                parse_sql(&query).map_err(|e| build_error!("ERR_SQL_PARSE", error = e))?;
            match request {
                SqlRequest::Read(q) => {
                    let result = QueryEngine::new(&col_mgr).execute_query(q).await?;
                    println!("{}", json::serialize_to_string_pretty(&result.documents)?);
                }
                SqlRequest::Write(ops) => {
                    tx_mgr.execute_smart(ops).await?;
                    user_success!("JSONDB_SQL_TX_SUCCESS", json_value!({}));
                }
            }
        }
        JsondbCommands::Import { collection, path } => {
            let json: JsonValue = fs::read_json_async(&path).await?;
            let docs = if let Some(arr) = json.as_array() {
                arr.to_vec()
            } else {
                vec![json]
            };
            let count = docs.len();
            for doc in docs {
                col_mgr.insert_with_schema(&collection, doc).await?;
            }
            user_success!("JSONDB_IMPORT_SUCCESS", json_value!({ "count": count }));
        }
        JsondbCommands::ImportSchemas {
            source_domain,
            source_db,
        } => {
            let count = col_mgr.import_schemas(&source_domain, &source_db).await?;
            user_success!("JSONDB_SCHEMAS_IMPORTED", json_value!({ "count": count }));
        }
        JsondbCommands::Transaction { file } => {
            let json_val: JsonValue = fs::read_json_async(&file).await?;
            let reqs: Vec<TransactionRequest> = if json_val.is_array() {
                json::deserialize_from_value(json_val)?
            } else if let Some(ops) = json_val.get("operations") {
                json::deserialize_from_value(ops.clone())?
            } else {
                raise_error!(
                    "ERR_JSONDB_INVALID_FORMAT",
                    error = "Format transaction invalide."
                );
            };
            tx_mgr.execute_smart(reqs).await?;
            user_success!("JSONDB_TX_SUCCESS", json_value!({}));
        }
        _ => {}
    }
    Ok(())
}

async fn parse_data(input: &str) -> RaiseResult<JsonValue> {
    if let Some(path_str) = input.strip_prefix('@') {
        fs::read_json_async(Path::new(path_str)).await
    } else {
        Ok(json::deserialize_from_str(input)?)
    }
}

// =========================================================================
// TESTS UNITAIRES (Conformité "Zéro Dette")
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: JsondbArgs,
    }

    #[test]
    #[serial_test::serial]
    fn verify_cli_structure() {
        use clap::CommandFactory;
        TestCli::command().debug_assert();
    }

    #[test]
    #[serial_test::serial]
    fn test_parse_create_index_defaults() -> RaiseResult<()> {
        let args = vec!["test", "create-index", "--collection", "u", "--field", "e"];
        let cli = TestCli::try_parse_from(args).map_err(|e| build_error!("ERR_TEST", error = e))?;
        if let JsondbCommands::CreateIndex { kind, .. } = cli.args.command {
            assert_eq!(kind, "hash");
            Ok(())
        } else {
            raise_error!("ERR_TEST_FAIL", error = "Parsing failed");
        }
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_parse_data_helper_robustness() -> RaiseResult<()> {
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        assert!(parse_data(r#"{"test":true}"#).await.is_ok());
        Ok(())
    }
}
