// FICHIER : src-tauri/tools/raise-cli/src/commands/jsondb.rs

use clap::{Args, Subcommand};

// --- IMPORTS RAISE ---
use raise::json_db::{
    collections::manager::{CollectionsManager, EntityIdentity},
    indexes::manager::IndexManager,
    query::{Condition, FilterOperator, Projection, Query, QueryEngine, QueryFilter},
    transactions::{manager::TransactionManager, TransactionRequest},
};
use raise::utils::prelude::*;

// 🎯 NOUVEAU : Import du contexte global CLI
use crate::CliContext;

// --- DÉFINITION DES ARGUMENTS ---

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

    RegisterOntology {
        #[arg(long, help = "Espace de nom (ex: arcadia, raise, oa)")]
        namespace: String,
        #[arg(long, help = "URI du fichier maître JSON-LD")]
        uri: String,
        #[arg(long, help = "Version sémantique exigée (ex: 1.1.0)")]
        version: String,
    },

    // --- DB & COLLECTIONS ---
    CreateDb {
        #[arg(
            long,
            required = true,
            help = "URI du schéma d'index obligatoire (ex: db://_system/_system/schemas/v1/db/index-mbse.schema.json)"
        )]
        schema: String,

        #[arg(
            long,
            required = true,
            help = "Rôle architectural et sémantique de la base (ex: system, raise, modeling, simulation...)"
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

    let active_domain = &ctx.active_domain;
    let active_db = &ctx.active_db;

    let col_mgr = CollectionsManager::new(storage, active_domain, active_db);
    let mut idx_mgr = IndexManager::new(storage, active_domain, active_db);
    let tx_mgr = TransactionManager::new(storage, active_domain, active_db);

    // Feedback contextuel
    if ctx.config.core.log_level == "debug" || ctx.config.core.log_level == "trace" {
        user_info!("JSONDB_CTX_ROOT", json_value!({ "path": root_dir }));
        user_info!(
            "JSONDB_CTX_SPACE",
            json_value!({
                "space": active_domain,
                "db": active_db,
                "action": "load_context"
            })
        );
    }

    if !matches!(
        args.command,
        JsondbCommands::CreateDb { .. } | JsondbCommands::Usage
    ) && !storage.config.db_root(active_domain, active_db).exists()
    {
        raise_error!(
            "ERR_DB_NOT_FOUND",
            error = format!("La base de données '{active_domain}/{active_db}' n'existe pas."),
            context = json_value!({
                "hint": format!("Initialisez d'abord la base avec : raise-cli jsondb --domain {} --db {} create-db --schema <URI>", active_domain, active_db)
            })
        );
    }

    match args.command {
        JsondbCommands::Usage => { /* Géré plus haut */ }

        // --- GESTION DES SCHÉMAS (DDL) ---
        JsondbCommands::ListSchemas => {
            let schemas = col_mgr.list_schemas().await?;
            user_success!(
                "JSONDB_SCHEMAS_LISTED",
                json_value!({ "space": active_domain, "db": active_db, "count": schemas.len() })
            );
            println!("{}", json::serialize_to_string_pretty(&schemas)?);
        }
        JsondbCommands::CreateSchema { name, schema } => {
            let schema_val = parse_data(&schema).await?;
            col_mgr.create_schema_def(&name, schema_val).await?;
            user_success!(
                "JSONDB_SCHEMA_CREATED",
                json_value!({ "schema": name, "status": "created" })
            );
        }
        JsondbCommands::DropSchema { name } => {
            col_mgr.drop_schema_def(&name).await?;
            user_success!(
                "JSONDB_SCHEMA_DROPPED",
                json_value!({ "schema": name, "status": "dropped" })
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
                json_value!({ "schema": name, "property": property, "status": "added" })
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
                json_value!({ "schema": name, "property": property, "status": "altered" })
            );
        }
        JsondbCommands::DropSchemaProperty { name, property } => {
            col_mgr.drop_schema_property(&name, &property).await?;
            user_success!(
                "JSONDB_SCHEMA_PROP_DROPPED",
                json_value!({ "schema": name, "property": property, "status": "dropped" })
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
                json_value!({
                    "namespace": namespace,
                    "uri": uri,
                    "version": version,
                    "status": "registered_in_dna"
                })
            );
        }
        JsondbCommands::CreateDb { schema, db_role } => {
            user_info!(
                "SYS_INFO",
                "Initialisation stricte de la base de données..."
            );

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            let created = manager.create_db_with_schema(&schema).await?;

            if created {
                let sys_path = ctx
                    .storage
                    .config
                    .db_root(&ctx.active_domain, &ctx.active_db)
                    .join("_system.json");

                if fs::exists_async(&sys_path).await {
                    if let Ok(mut sys_json) = fs::read_json_async::<JsonValue>(&sys_path).await {
                        if let Some(obj) = sys_json.as_object_mut() {
                            obj.insert("db_role".to_string(), JsonValue::String(db_role.clone()));
                        }
                        // On sauvegarde l'index enrichi
                        let _ = fs::write_json_atomic_async(&sys_path, &sys_json).await;
                    }
                }

                user_info!(
                    "JSONDB_INIT_SUCCESS",
                    json_value!({ "space": active_domain, "db": active_db })
                );
            } else {
                user_info!(
                    "JSONDB_EXISTS",
                    json_value!({ "space": active_domain, "db": active_db })
                );
            }
        }
        JsondbCommands::DropDb { force } => {
            if !force {
                // 🎯 Utilisation stricte avec contexte JSON vide
                user_error!("JSONDB_DROP_WARN", json_value!({}));
            } else if col_mgr.drop_db().await? {
                user_success!(
                    "JSONDB_DROP_SUCCESS",
                    json_value!({ "space": active_domain, "db": active_db, "action": "permanent_deletion" })
                );
            } else {
                user_error!(
                    "JSONDB_DROP_NOT_FOUND",
                    json_value!({ "space": active_domain, "db": active_db })
                );
            }
        }
        JsondbCommands::CreateCollection { name, schema } => {
            let Some(raw_schema) = schema else {
                raise_error!(
                    "ERR_CLI_MISSING_SCHEMA",
                    error = "REQUIRED_ARG_NOT_FOUND",
                    context = json_value!({
                        "action": "validate_command_args",
                        "param": "--schema",
                        "hint": "L'argument --schema est nécessaire pour initialiser la structure. Exemple: --schema user.json"
                    })
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
            user_success!(
                "JSONDB_COL_CREATED",
                json_value!({ "collection": name, "status": "active" })
            );
        }
        JsondbCommands::DropCollection { name } => {
            col_mgr.drop_collection(&name).await?;
            user_success!(
                "JSONDB_COL_DROPPED",
                json_value!({ "collection": name, "action": "cleanup" })
            );
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
            user_success!(
                "JSONDB_INDEX_CREATED",
                json_value!({ "collection": collection, "field": field, "type": kind })
            );
        }
        JsondbCommands::DropIndex { collection, field } => {
            idx_mgr.drop_index(&collection, &field).await?;
            // 🎯 Ajout d'un contexte JSON
            user_success!(
                "JSONDB_INDEX_DROPPED",
                json_value!({"collection": collection, "field": field})
            );
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
            user_success!(
                "JSONDB_LIST_SUCCESS",
                json_value!({
                    "collection": collection,
                    "count": result.total_count
                })
            );
            println!("{}", json::serialize_to_string_pretty(&result.documents)?);
        }
        JsondbCommands::Insert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let res = col_mgr.insert_with_schema(&collection, json_val).await?;

            let id = res.get("id").and_then(|v| v.as_str()).unwrap_or("?");
            user_success!(
                "JSONDB_INSERT_SUCCESS",
                json_value!({ "id": id, "status": "persisted" })
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
                json_value!({ "id": id, "action": "update" })
            );
        }
        JsondbCommands::Upsert { collection, data } => {
            let json_val = parse_data(&data).await?;
            let status = col_mgr.upsert_document(&collection, json_val).await?;
            user_success!(
                "JSONDB_UPSERT_SUCCESS",
                json_value!({ "status": format!("{:?}", status) })
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
                json_value!({ "collection": collection, "status": "deleted" })
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
            println!("{}", json::serialize_to_string_pretty(&result.documents)?);
        }
        JsondbCommands::Sql { query } => {
            use raise::json_db::query::sql::{parse_sql, SqlRequest};
            let sql_request = match parse_sql(&query) {
                Ok(req) => req,
                Err(e) => raise_error!(
                    "ERR_SQL_PARSE_FAILURE",
                    error = e,
                    context = json_value!({
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

                    println!("{}", json::serialize_to_string_pretty(&result.documents)?);
                }
                SqlRequest::Write(requests) => {
                    tx_mgr.execute_smart(requests).await?;
                    user_success!(
                        "JSONDB_SQL_TX_SUCCESS",
                        json_value!({"status": "committed"})
                    );
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

            // 🎯 CORRECTIF : On sauvegarde le compte avant de consommer (move) le vecteur
            let docs_count = docs.len();

            for doc in docs {
                col_mgr.insert_with_schema(&collection, doc).await?;
            }

            // 🎯 On utilise docs_count au lieu de docs.len()
            user_success!(
                "JSONDB_IMPORT_SUCCESS",
                json_value!({"collection": collection, "docs_imported": docs_count})
            );
        }

        JsondbCommands::ImportSchemas {
            source_domain,
            source_db,
        } => {
            let count = col_mgr.import_schemas(&source_domain, &source_db).await?;
            user_success!(
                "JSONDB_SCHEMAS_IMPORTED",
                json_value!({ "count": count, "space": source_domain, "db": source_db })
            );
        }

        JsondbCommands::Transaction { file } => {
            let json_val: JsonValue = fs::read_json_async(&file).await?;

            // 🎯 RÉSOLUTION : On vérifie le type AVANT de désérialiser
            // pour éviter de déclencher des logs d'erreurs inutiles.
            let reqs: Vec<TransactionRequest> = if json_val.is_array() {
                // Cas 1 : Tableau direct [...]
                json::deserialize_from_value::<Vec<TransactionRequest>>(json_val)?
            } else if json_val.get("operations").is_some() {
                // Cas 2 : Objet Wrapper { "operations": [...] }
                #[derive(Deserializable)]
                struct Wrapper {
                    operations: Vec<TransactionRequest>,
                }
                let w: Wrapper = json::deserialize_from_value(json_val)?;
                w.operations
            } else {
                // Cas 3 : Format inconnu
                raise_error!(
                    "ERR_JSONDB_INVALID_FORMAT",
                    error = "FORMAT_NOT_RECOGNIZED",
                    context = json_value!({
                        "action": "parse_transaction_file",
                        "hint": "Le fichier doit être soit un tableau d'opérations [...], soit un objet { \"operations\": [...] }."
                    })
                )
            };

            user_info!(
                "JSONDB_TX_START",
                json_value!({ "batch_size": reqs.len(), "mode": "atomic" })
            );
            tx_mgr.execute_smart(reqs).await?;
            // 🎯 Ajout d'un contexte JSON
            user_success!("JSONDB_TX_SUCCESS", json_value!({"status": "committed"}));
        }
    }
    Ok(())
}

// --- HELPERS ---

async fn parse_data(input: &str) -> RaiseResult<JsonValue> {
    if let Some(path_str) = input.strip_prefix('@') {
        let path = Path::new(path_str);
        let data = fs::read_json_async(path).await?;
        Ok(data)
    } else {
        Ok(json::deserialize_from_str(input)?)
    }
}

fn print_examples() {
    user_info!("JSONDB_USAGE_TITLE", json_value!({}));
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
        let res: RaiseResult<JsonValue> = json::deserialize_from_str(json);
        assert!(res.is_ok());
    }

    #[async_test]
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

    #[test]
    fn test_parse_register_ontology_command() {
        let args = vec![
            "test",
            "register-ontology",
            "--namespace",
            "arcadia",
            "--uri",
            "db://_system/bootstrap/schemas/v2/system/db/arcadia.jsonld",
            "--version",
            "1.1.0",
        ];
        let cli = TestCli::parse_from(args);
        match cli.args.command {
            JsondbCommands::RegisterOntology {
                namespace,
                uri,
                version,
            } => {
                assert_eq!(namespace, "arcadia");
                assert_eq!(
                    uri,
                    "db://_system/bootstrap/schemas/v2/system/db/arcadia.jsonld"
                );
                assert_eq!(version, "1.1.0");
            }
            _ => panic!("Parsing register-ontology failed"),
        }
    }
}
