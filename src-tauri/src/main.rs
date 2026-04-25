// FICHIER : src-tauri/src/main.rs

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use raise::utils::{context, prelude::*};
use tauri::Manager;

// --- IMPORTS RAISE ---
use raise::blockchain::{ConnectionProfile, FabricClient};
use raise::commands::{
    ai_commands, blockchain_commands, codegen_commands, cognitive_commands, dl_commands,
    genetics_commands, json_db_commands, model_commands, rules_commands, traceability_commands,
    training_commands, utils_commands, workflow_commands,
};

use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::jsonld::VocabularyRegistry;
use raise::json_db::migrations::migrator::Migrator;
use raise::json_db::migrations::{Migration, MigrationStep};
use raise::json_db::storage::{JsonDbConfig, StorageEngine};

use raise::plugins::manager::PluginManager;

// --- NOUVEAU : IMPORT DU KERNEL ---
use raise::kernel::state::RaiseKernelState;

// Structures d'état
use raise::commands::workflow_commands::WorkflowStore;
use raise::workflow_engine::executor::WorkflowExecutor;
use raise::workflow_engine::scheduler::WorkflowScheduler;

pub use raise::model_engine::types::ProjectModel;
use raise::AppState;

use raise::ai::graph_store::GraphStore;
use raise::commands::dl_commands::DlState;
use raise::spatial_engine;

#[allow(clippy::await_holding_lock)]
fn main() {
    // 1. INITIALISATION CONFIGURATION & LOGGING
    if let Err(e) = AppConfig::init() {
        kernel_fatal!(
            "Bootstrap du Système (Initialisation)",
            "Environment / AppConfig",
            e
        );
        terminate_process(1);
    }

    context::init_logging();
    user_info!("INF_RAISE_BOOT_START");

    tauri::Builder::default()
        // On ne manage plus le NativeLlmState ou le AiState isolés ici !
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_config = AppConfig::get();

            // 2. RÉSOLUTION DES POINTS DE MONTAGE SYSTÈME
            let db_root = match app_config.get_path("PATH_RAISE_DOMAIN") {
                Some(path) => path,
                None => {
                    user_error!(
                        "ERR_CONFIG_MISSING_PATH",
                        json_value!({"path": "PATH_RAISE_DOMAIN"})
                    );
                    terminate_process(1);
                }
            };

            if !db_root.exists() {
                if let Err(e) = fs::create_dir_all_sync(&db_root) {
                    user_error!(
                        "ERR_FS_DOMAIN_CREATION",
                        json_value!({"error": e.to_string()})
                    );
                }
            }

            let config = JsonDbConfig::new(db_root.clone());
            let storage = StorageEngine::new(config.clone())?;

            let system_domain = &app_config.mount_points.system.domain;
            let system_db = &app_config.mount_points.system.db;

            // ---------------------------------------------------------
            // 🛡️ MOTEUR DE RÉSILIENCE (WAL Crash Recovery)
            // ---------------------------------------------------------
            let wal_config = config.clone();
            let wal_storage = storage.clone();
            let wal_domain = system_domain.clone();
            let wal_db = system_db.clone();

            tauri::async_runtime::block_on(async move {
                match raise::json_db::transactions::wal::recover_pending_transactions(
                    &wal_config,
                    &wal_domain,
                    &wal_db,
                    &wal_storage,
                )
                .await
                {
                    Ok(count) if count > 0 => {
                        user_warn!(
                            "WRN_DB_CRASH_RECOVERED",
                            json_value!({"recovered_transactions": count})
                        );
                    }
                    Err(e) => {
                        user_error!(
                            "ERR_DB_RECOVERY_FAIL",
                            json_value!({"error": e.to_string()})
                        );
                    }
                    _ => {}
                }
            });

            // ---------------------------------------------------------
            // 🎯 BOOTSTRAP DU MOTEUR DE RÈGLES
            // ---------------------------------------------------------
            tauri::async_runtime::block_on(async {
                let manager = CollectionsManager::new(&storage, system_domain, system_db);
                if let Err(e) = raise::rules_engine::initialize_rules_engine(&manager).await {
                    user_error!(
                        "ERR_RULES_ENGINE_BOOT_FAIL",
                        json_value!({"error": e.to_string()})
                    );
                }
            });

            // ---------------------------------------------------------
            // 3. INITIALISATION SÉMANTIQUE (Bootstrapping "In-Index")
            // ---------------------------------------------------------
            tauri::async_runtime::spawn({
                let storage_reg = storage.clone();
                let domain_reg = system_domain.clone();
                let db_reg = system_db.clone();
                async move {
                    let db_manager = CollectionsManager::new(&storage_reg, &domain_reg, &db_reg);
                    if let Err(e) = VocabularyRegistry::init_from_db(&db_manager).await {
                        user_error!(
                            "ERR_ONTOLOGY_BOOTSTRAP_FAIL",
                            json_value!({"error": e.to_string()})
                        );
                    }
                }
            });

            // 4. GRAPH STORE
            let graph_path = db_root.join("graph_store");
            let graph_store_result = tauri::async_runtime::block_on(async {
                let manager = CollectionsManager::new(&storage, system_domain, system_db);
                GraphStore::new(graph_path, &manager).await
            });

            if let Ok(store) = graph_store_result {
                app.manage(store);
            }

            // 5. MIGRATIONS
            let _ = tauri::async_runtime::block_on(run_app_migrations(
                &storage,
                system_domain,
                system_db,
            ));

            // 6. INJECTION DES ÉTATS DE BASE
            let plugin_mgr = SharedRef::new(PluginManager::new(&storage, None));
            app.manage(config);
            app.manage(storage.clone());
            app.manage(plugin_mgr.clone());
            app.manage(context::SessionManager::new(SharedRef::new(
                storage.clone(),
            )));

            app.manage(SharedRef::new(AppState {
                model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
            }));
            app.manage(AsyncMutex::new(WorkflowStore::default()));
            app.manage(DlState::new());

            // BLOCKCHAIN
            raise::blockchain::ensure_innernet_state(app.handle(), "default");
            let default_fabric_profile = ConnectionProfile {
                name: "pending".into(),
                version: "1.0.0".into(),
                client: raise::blockchain::fabric::config::ClientConfig {
                    organization: "none".into(),
                    connection: None,
                },
                organizations: UnorderedMap::new(),
                peers: UnorderedMap::new(),
                certificate_authorities: UnorderedMap::new(),
            };
            app.manage(SharedRef::new(AsyncMutex::new(FabricClient::from_config(
                default_fabric_profile,
            ))));
            raise::blockchain::p2p::service::init_arcadia_network(app.handle().clone());

            // ====================================================================
            // 7. 🧠 LE NOYAU (KERNEL) : SÉQUENCE DE BOOT STRICTE ET UNIFIÉE
            // ====================================================================
            // On bloque le thread principal de Tauri pour garantir que l'application
            // ne s'ouvre que lorsque la VRAM est allouée et sécurisée.
            let kernel = tauri::async_runtime::block_on(async {
                RaiseKernelState::boot(SharedRef::new(storage.clone())).await
            })
            .expect("❌ Erreur fatale : Le Kernel n'a pas pu démarrer.");

            // Injection du Singleton dans le registre Tauri
            app.manage(kernel.clone());

            // ====================================================================
            // 8. WORKFLOW ENGINE (Dépend du Kernel)
            // ====================================================================
            if let Some(orch_ref) = kernel.orchestrator {
                let executor = WorkflowExecutor::new(orch_ref, plugin_mgr);
                tauri::async_runtime::block_on(async {
                    let wf_state = app.handle().state::<AsyncMutex<WorkflowStore>>();
                    let mut wf_store = wf_state.lock().await;
                    wf_store.scheduler = Some(WorkflowScheduler::new(executor));
                });
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            json_db_commands::jsondb_create_db,
            json_db_commands::jsondb_drop_db,
            json_db_commands::jsondb_create_collection,
            json_db_commands::jsondb_list_collections,
            json_db_commands::jsondb_drop_collection,
            json_db_commands::jsondb_create_index,
            json_db_commands::jsondb_drop_index,
            json_db_commands::jsondb_insert_document,
            json_db_commands::jsondb_get_document,
            json_db_commands::jsondb_update_document,
            json_db_commands::jsondb_delete_document,
            json_db_commands::jsondb_list_all,
            json_db_commands::jsondb_execute_query,
            json_db_commands::jsondb_execute_sql,
            json_db_commands::jsondb_evaluate_draft,
            json_db_commands::jsondb_init_demo_rules,
            model_commands::load_project_model,
            rules_commands::dry_run_rule,
            rules_commands::validate_model,
            ai_commands::ai_chat,
            ai_commands::ai_reset,
            ai_commands::ask_native_llm,
            ai_commands::ai_learn_text,
            ai_commands::ai_export_dataset,
            ai_commands::validate_arcadia_gnn,
            dl_commands::init_dl_model,
            dl_commands::run_dl_prediction,
            dl_commands::train_dl_step,
            dl_commands::save_dl_model,
            dl_commands::load_dl_model,
            training_commands::tauri_train_domain,
            cognitive_commands::cognitive_load_plugin,
            cognitive_commands::cognitive_run_plugin,
            cognitive_commands::cognitive_list_plugins,
            blockchain_commands::fabric_ping,
            blockchain_commands::fabric_submit_transaction,
            blockchain_commands::fabric_query_transaction,
            blockchain_commands::fabric_get_history,
            blockchain_commands::vpn_network_status,
            blockchain_commands::vpn_connect,
            blockchain_commands::vpn_disconnect,
            blockchain_commands::vpn_list_peers,
            blockchain_commands::vpn_add_peer,
            blockchain_commands::vpn_ping_peer,
            blockchain_commands::vpn_check_installation,
            blockchain_commands::arcadia_broadcast_mutation,
            blockchain_commands::arcadia_get_sync_status,
            blockchain_commands::arcadia_get_ledger_info,
            genetics_commands::run_architecture_optimization,
            genetics_commands::debug_genetics_ping,
            codegen_commands::generate_source_code,
            traceability_commands::analyze_impact,
            traceability_commands::run_compliance_audit,
            traceability_commands::get_traceability_matrix,
            traceability_commands::get_element_neighbors,
            utils_commands::get_app_info,
            utils_commands::session_login,
            utils_commands::session_logout,
            utils_commands::session_get,
            workflow_commands::compile_mission,
            workflow_commands::register_workflow,
            workflow_commands::start_workflow,
            workflow_commands::resume_workflow,
            workflow_commands::get_workflow_state,
            workflow_commands::set_sensor_value,
            spatial_engine::get_spatial_topology
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

async fn run_app_migrations(storage: &StorageEngine, space: &str, db: &str) -> RaiseResult<()> {
    let migrator = Migrator::new(storage, space, db);
    let schema_uri = "db://_system/_system/schemas/v1/db/generic.schema.json".to_string();

    let migrations = vec![
        Migration {
            id: "init_001_core_collections".to_string(),
            version: "1.0.0".to_string(),
            description: "Init Core".to_string(),
            up: vec![
                MigrationStep::CreateCollection {
                    name: "articles".to_string(),
                    schema: JsonValue::String(schema_uri.clone()),
                },
                MigrationStep::CreateCollection {
                    name: "systems".to_string(),
                    schema: JsonValue::String(schema_uri.clone()),
                },
                MigrationStep::CreateCollection {
                    name: "exchange_items".to_string(),
                    schema: JsonValue::String(schema_uri),
                },
            ],
            down: vec![],
            applied_at: None,
        },
        Migration {
            id: "idx_001_articles_title".to_string(),
            version: "1.1.0".to_string(),
            description: "Idx title".to_string(),
            up: vec![MigrationStep::CreateIndex {
                collection: "articles".to_string(),
                fields: vec!["title".to_string()],
            }],
            down: vec![],
            applied_at: None,
        },
    ];

    match migrator.run_migrations(migrations).await {
        Ok(_) => Ok(()),
        Err(e) => raise_error!("ERR_MIGRATION_FAIL", error = e.to_string()),
    }
}

// ============================================================================
// TESTS UNITAIRES (Conformité & Résilience Mount Points)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    #[serial_test::serial]
    async fn test_vocabulary_registry_db_init_robustness() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let space = "test_space";
        let db = "test_db";
        let manager = CollectionsManager::new(&sandbox.storage, space, db);
        DbSandbox::mock_db(&manager).await?;

        // 🎯 FIX STRICT SCHEMA : On crée explicitement la collection avant l'insertion brute
        manager
            .create_collection(
                "_ontologies",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // 1. Injection d'une ontologie mockée dans la collection système _ontologies
        let ont_json = json_value!({
            "_id": "ontology_core",
            "@context": { "test": "http://test#" },
            "@graph": []
        });
        manager.insert_raw("_ontologies", &ont_json).await?;

        // 2. Référencement de l'ontologie dans l'index (_system.json)
        let sys_path = sandbox
            .storage
            .config
            .db_root(space, db)
            .join("_system.json");
        let mut sys_doc: JsonValue = fs::read_json_async(&sys_path).await?;
        sys_doc["ontologies"]["core"] = json_value!({ "uri": "db://...", "version": "1.0" });
        fs::write_json_atomic_async(&sys_path, &sys_doc).await?;

        // 3. Action : Bootstrapping In-Index
        VocabularyRegistry::init_from_db(&manager).await?;

        let registry = VocabularyRegistry::global()?;
        assert!(
            registry.get_default_context().contains_key("test"),
            "L'ontologie n'a pas été chargée depuis la collection système."
        );

        Ok(())
    }

    #[async_test]
    async fn test_migrations_list_integrity() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;

        let manager = CollectionsManager::new(&sandbox.storage, space, db);
        DbSandbox::mock_db(&manager).await.expect("Init index fail");

        run_app_migrations(&sandbox.storage, space, db).await?;
        Ok(())
    }

    /// Résilience du point de montage système
    #[async_test]
    async fn test_mount_point_resolution_resilience() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        assert!(
            !config.mount_points.system.domain.is_empty(),
            "La partition système (domain) doit être définie"
        );
        assert!(
            !config.mount_points.system.db.is_empty(),
            "La base système (db) doit être définie"
        );
        Ok(())
    }
}
