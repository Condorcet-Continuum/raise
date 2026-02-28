// FICHIER : src-tauri/src/commands/json_db_commands.rs

use crate::utils::prelude::*;

use crate::json_db::collections::manager::{self, CollectionsManager};
use crate::json_db::query::{sql::SqlRequest, Query, QueryEngine, QueryResult};
use crate::json_db::schema::SchemaRegistry;
use crate::json_db::storage::{file_storage, StorageEngine};
use crate::json_db::transactions::manager::TransactionManager;
use tauri::{command, State};

// Helper pour instancier le manager rapidement
fn mgr<'a>(
    storage: &'a State<'_, StorageEngine>,
    space: &str,
    db: &str,
) -> RaiseResult<CollectionsManager<'a>> {
    Ok(CollectionsManager::new(storage, space, db))
}

// --- GESTION DATABASE ---

#[command]
pub async fn jsondb_create_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<bool> {
    // 1. Création physique (Système de fichiers)
    if let Err(e) = file_storage::create_db(&storage.config, &space, &db).await {
        raise_error!(
            "ERR_DB_FILESYSTEM_CREATION_FAILED",
            error = e,
            context = json!({
                "action": "create_db_directory",
                "space": space,
                "db": db,
                "data_root": storage.config.data_root,
                "hint": "Impossible de créer les répertoires sur le disque. Vérifiez les permissions d'écriture."
            })
        );
    }

    // 2. Récupération du manager (Accès logique)
    let manager = mgr(&storage, &space, &db)?;

    // 3. Initialisation des tables/schémas internes
    match manager.init_db().await {
        Ok(_) => Ok(true),
        Err(e) => raise_error!(
            "ERR_DB_LOGICAL_INIT_FAILED",
            error = e,
            context = json!({
                "action": "initialize_db_metadata",
                "db": db,
                "hint": "Le répertoire a été créé mais l'initialisation des métadonnées a échoué."
            })
        ),
    }
}

#[command]
pub async fn jsondb_drop_db(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<bool> {
    // 1. Exécution de la suppression physique avec capture d'erreur système
    match file_storage::drop_db(&storage.config, &space, &db, file_storage::DropMode::Hard).await {
        Ok(_) => Ok(true),
        Err(e) => raise_error!(
            "ERR_DB_DROP_FAILED",
            error = e,
            context = json!({
                "action": "drop_database_hard",
                "space": space,
                "db": db,
                "mode": "Hard",
                "hint": "Échec de la suppression physique. Un fichier est peut-être utilisé par un autre processus ou les permissions sont insuffisantes."
            })
        ),
    }
}

// --- GESTION COLLECTIONS ---

#[command]
pub async fn jsondb_create_collection(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    schema_uri: Option<String>,
) -> RaiseResult<bool> {
    let manager = mgr(&storage, &space, &db)?;

    // 1. Capture des noms pour le diagnostic
    let coll_name = collection.clone();
    let uri_info = schema_uri.clone().unwrap_or_else(|| "None".to_string());

    // 2. Création avec validation du schéma optionnel
    match manager.create_collection(&collection, schema_uri).await {
        Ok(_) => Ok(true),
        Err(e) => raise_error!(
            "ERR_DB_COLLECTION_CREATION_FAILED",
            error = e,
            context = json!({
                "action": "create_collection",
                "collection": coll_name,
                "schema_uri": uri_info,
                "hint": "Impossible de créer la collection. Vérifiez si le nom est déjà utilisé ou si l'URI du schéma est accessible."
            })
        ),
    }
}

#[command]
pub async fn jsondb_list_collections(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<Vec<String>> {
    // 1. Accès au manager (gestion automatique du verrouillage)
    let manager = mgr(&storage, &space, &db)?;

    // 2. Récupération de la liste avec capture du contexte spatial
    match manager.list_collections().await {
        Ok(collections) => Ok(collections),
        Err(e) => raise_error!(
            "ERR_DB_LIST_COLLECTIONS_FAILED",
            error = e,
            context = json!({
                "action": "list_collections",
                "space": space,
                "db": db,
                "hint": "Impossible de lire la structure de la base. Vérifiez si le répertoire db existe dans le data_root."
            })
        ),
    }
}

#[command]
pub async fn jsondb_drop_collection(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
) -> RaiseResult<bool> {
    let manager = mgr(&storage, &space, &db)?;

    // 1. Capture du nom pour le contexte d'erreur
    let coll_name = collection.clone();

    // 2. Exécution de la suppression
    match manager.drop_collection(&collection).await {
        Ok(_) => Ok(true),
        Err(e) => raise_error!(
            "ERR_DB_COLLECTION_DROP_FAILED",
            error = e,
            context = json!({
                "action": "drop_collection",
                "collection": coll_name,
                "db": db,
                "space": space,
                "hint": "Échec de la suppression. La collection est peut-être verrouillée par un autre processus ou déjà supprimée."
            })
        ),
    }
}
// --- GESTION INDEXES ---

#[command]
pub async fn jsondb_create_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
    kind: String,
) -> RaiseResult<bool> {
    let manager = mgr(&storage, &space, &db)?;

    // 1. Capture du contexte pour le diagnostic technique
    let coll_name = collection.clone();

    // 2. Création de l'index avec capture d'erreurs de contraintes
    match manager.create_index(&collection, &field, &kind).await {
        Ok(_) => Ok(true),
        Err(e) => raise_error!(
            "ERR_DB_INDEX_CREATION_FAILED",
            error = e,
            context = json!({
                "action": "create_index",
                "collection": coll_name,
                "field": field,
                "kind": kind,
                "hint": "Impossible de créer l'index. Vérifiez si le champ existe dans le schéma et si le type d'index (kind) est supporté."
            })
        ),
    }
}

#[command]
pub async fn jsondb_drop_index(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    field: String,
) -> RaiseResult<bool> {
    let manager = mgr(&storage, &space, &db)?;

    // 1. Capture du contexte pour identifier l'optimisation supprimée
    let coll_name = collection.clone();
    let field_name = field.clone();

    // 2. Suppression avec gestion des erreurs I/O
    match manager.drop_index(&collection, &field).await {
        Ok(_) => Ok(true),
        Err(e) => raise_error!(
            "ERR_DB_INDEX_DROP_FAILED",
            error = e,
            context = json!({
                "action": "drop_index",
                "collection": coll_name,
                "field": field_name,
                "db": db,
                "hint": "Impossible de supprimer l'index. Le fichier d'index est peut-être verrouillé ou déjà supprimé."
            })
        ),
    }
}

// --- MOTEUR DE RÈGLES ---

#[command]
pub async fn jsondb_evaluate_draft(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    mut doc: Value,
) -> RaiseResult<Value> {
    // 1. Registry Init
    let registry = match SchemaRegistry::from_db(&storage.config, &space, &db).await {
        Ok(reg) => reg,
        Err(e) => {
            raise_error!(
                "ERR_SCHEMA_REGISTRY_INIT_FAILED",
                error = e,
                context = json!({ "space": space, "db": db })
            );
        }
    };

    // 2. Extraction du Schéma
    let meta_path = storage
        .config
        .db_collection_path(&space, &db, &collection)
        .join("_meta.json");

    let schema_uri = if meta_path.exists() {
        // 1. Lecture du fichier sans map_err
        let content = match std::fs::read_to_string(&meta_path) {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_DB_META_READ_FAIL",
                error = e,
                context = json!({ "path": meta_path, "collection": collection })
            ),
        };

        // 2. Parsing JSON sans map_err
        let meta: Value = match serde_json::from_str(&content) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_DB_META_PARSE_FAIL",
                error = e,
                context = json!({ "content": content, "collection": collection })
            ),
        };

        // 3. Extraction sécurisée
        meta.get("schema")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default()
    } else {
        raise_error!(
            "ERR_DB_COLLECTION_NOT_INITIALIZED",
            context = json!({
                "collection": collection,
                "hint": "La collection n'a pas de fichier .meta.json. Elle doit être initialisée avant toute opération."
            })
        );
    };

    // Si pas de schéma, on valide par défaut
    if schema_uri.is_empty() {
        return Ok(doc);
    }

    // 3. Application des Règles (Correction du Type &str)
    let manager = mgr(&storage, &space, &db)?;

    // On passe &schema_uri car String implémente Deref pour str
    match manager::apply_business_rules(
        &manager,
        &collection,
        &mut doc,
        None, // Version
        &registry,
        &schema_uri, // <--- LA RÉPARATION : expected &str, found &str
    )
    .await
    {
        Ok(_) => Ok(doc),
        Err(e) => raise_error!(
            "ERR_BUSINESS_RULES_VIOLATION",
            error = e,
            context = json!({ "collection": collection, "schema": schema_uri })
        ),
    }
}

// --- CRUD DOCUMENTS ---

#[command]
pub async fn jsondb_insert_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    document: Value,
) -> RaiseResult<Value> {
    let manager = mgr(&storage, &space, &db)?;
    // 1. On capture le nom de la collection pour le contexte
    let collection_name = collection.clone();
    match manager.insert_with_schema(&collection, document).await {
        Ok(id) => Ok(id),
        Err(e) => raise_error!(
            "ERR_DB_INSERT_VALIDATION_FAILED",
            error = e,
            context = json!({
                "action": "insert_document",
                "collection": collection_name,
                "hint": "Le document ne respecte pas le schéma défini pour cette collection ou le stockage est verrouillé."
            })
        ),
    }
}

#[command]
pub async fn jsondb_update_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
    document: Value,
) -> RaiseResult<Value> {
    let manager = mgr(&storage, &space, &db)?;
    // 1. Capture du contexte pour le diagnostic
    let coll_name = collection.clone();
    let doc_id = id.clone();

    match manager.update_document(&collection, &id, document).await {
        // On transforme le () en une Value de succès
        Ok(_) => Ok(json!({
            "status": "success",
            "message": format!("Document {} mis à jour dans {}", doc_id, coll_name)
        })),
        Err(e) => raise_error!(
            "ERR_DB_UPDATE_FAILED",
            error = e,
            context = json!({
                "action": "update_document",
                "collection": coll_name,
                "document_id": doc_id
            })
        ),
    }
}

#[command]
pub async fn jsondb_get_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
) -> RaiseResult<Option<Value>> {
    let manager = mgr(&storage, &space, &db)?;
    // 1. Capture du contexte pour le diagnostic
    let coll_name = collection.clone();
    let doc_id = id.clone();

    match manager.get_document(&collection, &id).await {
        Ok(doc) => Ok(doc),
        Err(e) => raise_error!(
            "ERR_DB_DOCUMENT_NOT_FOUND",
            error = e,
            context = json!({
                "action": "fetch_document",
                "collection": coll_name,
                "document_id": doc_id,
                "hint": "Le document est peut-être absent ou l'accès au fichier JSON a été refusé."
            })
        ),
    }
}

#[command]
pub async fn jsondb_delete_document(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
    id: String,
) -> RaiseResult<bool> {
    let manager = mgr(&storage, &space, &db)?;

    // On garde le contexte pour l'erreur au cas où
    let coll_name = collection.clone();
    let doc_id = id.clone();

    match manager.delete_document(&collection, &id).await {
        // CORRECTION : On renvoie true pour correspondre à RaiseResult<bool>
        Ok(_) => Ok(true),

        Err(e) => raise_error!(
            "ERR_DB_DELETE_FAILED",
            error = e,
            context = json!({
                "action": "delete_document",
                "collection": coll_name,
                "document_id": doc_id,
                "hint": "Impossible de supprimer le document. Vérifiez les permissions du répertoire data_root."
            })
        ),
    }
}

#[command]
pub async fn jsondb_list_all(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    collection: String,
) -> RaiseResult<Vec<Value>> {
    let manager = mgr(&storage, &space, &db)?;

    let documents = match manager.list_all(&collection).await {
        Ok(docs) => docs,
        Err(e) => raise_error!(
            "ERR_DB_LIST_ALL_FAIL",
            error = e,
            context = json!({
                "collection": collection,
                "action": "list_all_documents",
                "hint": "Échec de lecture de la base de données. Vérifiez l'existence du dossier de collection et les permissions système."
            })
        ),
    }; // On a extrait les documents avec succès

    Ok(documents) // <--- C'est cette ligne qui manquait pour satisfaire le compilateur !
}

// --- REQUÊTES (MODIFIÉ POUR INSERT SQL) ---

#[command]
pub async fn jsondb_execute_query(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    query: Query,
) -> RaiseResult<QueryResult> {
    let manager = mgr(&storage, &space, &db)?;
    let engine = QueryEngine::new(&manager);
    let query_context = format!("{:?}", query);

    match engine.execute_query(query).await {
        Ok(results) => Ok(results),
        Err(e) => raise_error!(
            "ERR_DB_QUERY_EXECUTION_FAILED",
            error = e,
            context = json!({
                "action": "execute_query",
                "query_preview": query_context,
                "hint": "La requête a échoué. Vérifiez la syntaxe des filtres et assurez-vous que les index sont à jour."
            })
        ),
    }
}

#[command]
pub async fn jsondb_execute_sql(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
    sql: String,
) -> RaiseResult<QueryResult> {
    let manager = mgr(&storage, &space, &db)?;

    // 1. Parsing SQL avec capture d'erreur de syntaxe
    let request = match crate::json_db::query::sql::parse_sql(&sql) {
        Ok(req) => req,
        Err(e) => raise_error!(
            "ERR_SQL_PARSE_FAILED",
            error = e,
            context = json!({
                "action": "parse_sql_query",
                "query_preview": sql,
                "hint": "La syntaxe SQL est incorrecte ou n'est pas supportée par le parseur Arcadia."
            })
        ),
    };

    match request {
        // CAS LECTURE (SELECT)
        SqlRequest::Read(query) => {
            let engine = QueryEngine::new(&manager);
            match engine.execute_query(query).await {
                Ok(res) => Ok(res),
                Err(e) => raise_error!(
                    "ERR_SQL_READ_EXECUTION",
                    error = e,
                    context = json!({
                        "action": "execute_sql_read",
                        "space": space,
                        "db": db
                    })
                ),
            }
        }
        // CAS ÉCRITURE (INSERT / UPDATE)
        SqlRequest::Write(requests) => {
            let tx_mgr = TransactionManager::new(&storage.config, &space, &db);

            if let Err(e) = tx_mgr.execute_smart(requests).await {
                raise_error!(
                    "ERR_SQL_WRITE_TRANSACTION",
                    error = e,
                    context = json!({
                        "action": "execute_sql_write",
                        "hint": "L'écriture SQL a échoué. Vérifiez les contraintes de schéma ou les verrous de base de données."
                    })
                );
            }

            Ok(QueryResult {
                documents: vec![],
                total_count: 0,
                limit: None,
                offset: None,
            })
        }
    }
}

// --- UTILITAIRES DÉMO ---

#[command]
pub async fn jsondb_init_demo_rules(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<()> {
    let mgr = mgr(&storage, &space, &db)?;

    // 1. Initialisation de la DB
    if let Err(e) = mgr.init_db().await {
        raise_error!(
            "ERR_DB_INIT_FAIL",
            error = e,
            context = json!({ "space": space, "db": db, "action": "init_db" })
        );
    }

    // 2. Création de la collection
    if let Err(e) = mgr.create_collection("users", None).await {
        raise_error!(
            "ERR_DB_CREATE_COLLECTION_FAIL",
            error = e,
            context = json!({ "collection": "users", "action": "setup_dev_env" })
        );
    }

    // 3. Insertion du document de test
    let user_doc = json!({ "id": "u_dev", "name": "Alice Dev", "tjm": 500.0 });
    if let Err(e) = mgr.insert_raw("users", &user_doc).await {
        raise_error!(
            "ERR_DB_INSERT_FAIL",
            error = e,
            context = json!({
                "collection": "users",
                "doc_id": "u_dev",
                "hint": "L'insertion du profil dev a échoué. Vérifiez si l'ID existe déjà ou si le schéma est respecté."
            })
        );
    }
    let schema_content = json!({
        "type": "object",
        "properties": {
            "user_id": { "type": "string" },
            "days": { "type": "number" },
            "created_at": { "type": "string" },
            "total": { "type": "number" },
            "due_at": { "type": "string" },
            "ref": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "calc_total_lookup",
                "target": "total",
                "expr": {
                    "mul": [
                        { "var": "days" },
                        { "lookup": { "collection": "users", "id": { "var": "user_id" }, "field": "tjm" } }
                    ]
                }
            },
            {
                "id": "calc_due_date",
                "target": "due_at",
                "expr": { "date_add": { "date": { "var": "created_at" }, "days": { "val": 30 } } }
            },
            {
                "id": "gen_ref",
                "target": "ref",
                "expr": {
                    "concat": [
                        { "val": "INV-" },
                        { "upper": { "var": "user_id" } },
                        { "val": "-" },
                        { "var": "total" }
                    ]
                }
            }
        ]
    });

    let schema_path = storage
        .config
        .db_schemas_root(&space, &db)
        .join("v1/invoices/default.json");
    if let Some(parent) = schema_path.parent() {
        // On remplace le map_err par un match ou un if let Err explicite
        if let Err(e) = std::fs::create_dir_all(parent) {
            raise_error!(
                "ERR_FS_DIR_CREATION_FAIL",
                error = e,
                context = json!({
                    "path": parent,
                    "action": "create_schema_directory",
                    "hint": "Impossible de créer le dossier parent pour le schéma. Vérifiez les permissions d'écriture sur le disque."
                })
            );
        }
    }
    // 1. Sérialisation sécurisée
    let pretty_json = match serde_json::to_string_pretty(&schema_content) {
        Ok(s) => s,
        Err(e) => raise_error!(
            "ERR_SERIALIZATION_FAIL",
            error = e,
            context = json!({
                "target": "schema_content",
                "action": "pretty_print_json",
                "hint": "Le contenu du schéma contient des types de données non sérialisables par Serde."
            })
        ),
    };

    // 2. Écriture disque sans map_err
    if let Err(e) = std::fs::write(&schema_path, pretty_json) {
        raise_error!(
            "ERR_FS_WRITE_FAIL",
            error = e,
            context = json!({
                "path": schema_path,
                "action": "write_schema_file",
                "hint": "L'écriture du fichier de schéma a échoué. Vérifiez l'espace disque ou les verrous de fichiers."
            })
        );
    }

    let schema_uri = format!("db://{}/{}/schemas/v1/invoices/default.json", space, db);
    if let Err(e) = mgr.create_collection("invoices", Some(schema_uri)).await {
        raise_error!(
            "ERR_DB_COLLECTION_CREATION_FAIL",
            error = e,
            context = json!({
                "collection": "invoices",
                "action": "initialize_invoices_storage",
                "hint": "Échec de création de la collection. Vérifiez si le schéma URI est accessible ou si la collection existe déjà avec des paramètres différents."
            })
        );
    }

    Ok(())
}

#[command]
pub async fn jsondb_init_model_rules(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<()> {
    let mgr = mgr(&storage, &space, &db)?;
    // Initialisation du moteur de stockage
    if let Err(e) = mgr.init_db().await {
        raise_error!(
            "ERR_DB_INIT_FAIL",
            error = e,
            context = json!({
                "action": "initialize_storage_engine",
                "hint": "Le moteur de base de données n'a pas pu démarrer. Vérifiez les permissions du dossier de stockage et l'espace disque disponible."
            })
        );
    }

    let schema_content = json!({
        "type": "object",
        "properties": {
            "name": { "type": "string" },
            "parent_pkg": { "type": "string" },
            "description": { "type": "string" },
            "full_path": { "type": "string" },
            "compliance": { "type": "string" }
        },
        "x_rules": [
            {
                "id": "compute_path",
                "target": "full_path",
                "expr": {
                    "concat": [
                        { "var": "parent_pkg" },
                        { "val": "::" },
                        { "var": "name" }
                    ]
                }
            },
            {
                "id": "check_naming",
                "target": "compliance",
                "expr": {
                    "if": {
                        "condition": {
                            "regex_match": {
                                "value": { "var": "name" },
                                "pattern": { "val": "^LF_[A-Z0-9_]+$" }
                            }
                        },
                        "then_branch": { "val": "✅ VALIDE" },
                        "else_branch": { "val": "❌ NON_CONFORME (Doit commencer par LF_ et être en MAJ)" }
                    }
                }
            }
        ]
    });

    let schema_path = storage
        .config
        .db_schemas_root(&space, &db)
        .join("v1/la/functions.json");
    // 1. Création sécurisée du répertoire parent
    if let Some(parent) = schema_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            raise_error!(
                "ERR_FS_DIR_CREATION_FAIL",
                error = e,
                context = json!({
                    "path": parent,
                    "action": "ensure_schema_directory",
                    "hint": "Vérifiez les permissions d'écriture sur le disque pour le dossier parent."
                })
            );
        }
    }

    // 2. Sérialisation sécurisée (on remplace le unwrap)
    let pretty_json = match serde_json::to_string_pretty(&schema_content) {
        Ok(s) => s,
        Err(e) => raise_error!(
            "ERR_SERIALIZATION_FAIL",
            error = e,
            context = json!({
                "target": "schema_content",
                "hint": "Le contenu du schéma contient des types incompatibles avec la sérialisation JSON."
            })
        ),
    };

    // 3. Écriture disque sans map_err
    if let Err(e) = std::fs::write(&schema_path, pretty_json) {
        raise_error!(
            "ERR_FS_WRITE_FAIL",
            error = e,
            context = json!({
                "path": schema_path,
                "action": "persist_schema_file",
                "hint": "Échec de l'écriture. Vérifiez l'espace disque ou si le fichier est déjà ouvert par un autre processus."
            })
        );
    }

    let schema_uri = format!("db://{}/{}/schemas/v1/la/functions.json", space, db);
    let _ = mgr
        .create_collection("logical_functions", Some(schema_uri))
        .await;

    Ok(())
}
