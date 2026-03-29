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

// --- IMPORT IA NATIF ---
use raise::ai::llm::candle_engine::CandleLlmEngine;
use raise::ai::llm::NativeLlmState;

use raise::json_db::collections::manager::CollectionsManager;
use raise::json_db::jsonld::VocabularyRegistry;
use raise::json_db::migrations::migrator::Migrator;
use raise::json_db::migrations::{Migration, MigrationStep};
use raise::json_db::storage::{JsonDbConfig, StorageEngine};

use raise::plugins::manager::PluginManager;

// Structures d'état
use raise::commands::ai_commands::AiState;
use raise::commands::workflow_commands::WorkflowStore;
use raise::workflow_engine::executor::WorkflowExecutor;
use raise::workflow_engine::scheduler::WorkflowScheduler;

pub use raise::model_engine::types::ProjectModel;
use raise::AppState;

use raise::ai::graph_store::GraphStore;
use raise::ai::orchestrator::AiOrchestrator;
use raise::model_engine::loader::ModelLoader;

use raise::commands::dl_commands::DlState;

use raise::spatial_engine;

#[allow(clippy::await_holding_lock)]
fn main() {
    if let Err(e) = AppConfig::init() {
        eprintln!("❌ Erreur fatale de configuration : {}", e);
        std::process::exit(1);
    }
    println!("🚀 Démarrage de RAISE...");
    context::init_logging();
    let _config = AppConfig::get();

    tauri::Builder::default()
        .manage(NativeLlmState(SyncMutex::new(None::<CandleLlmEngine>)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 2. CONFIG DOMAINE & STOCKAGE
            let app_config = AppConfig::get();
            let db_root = app_config
                .get_path("PATH_RAISE_DOMAIN")
                .expect("❌ ERREUR FATALE: PATH_RAISE_DOMAIN introuvable dans la configuration !");
            if !db_root.exists() {
                fs::create_dir_all_sync(&db_root)?;
            }
            let config = JsonDbConfig::new(db_root.clone());
            let storage = StorageEngine::new(config.clone());

            let default_space = &app_config.system_domain;
            let default_db = &app_config.system_db;

            // 🎯 3. INITIALISATION DYNAMIQUE DES ONTOLOGIES (Bootstrapping)
            let ontology_root = db_root.join("_system/ontology");
            tauri::async_runtime::spawn(async move {
                println!(
                    "📂 [Ontology] Initialisation sémantique depuis {:?}",
                    ontology_root
                );
                // 🎯 Appel au nouveau scanner récursif agnostique
                if let Err(e) = VocabularyRegistry::init(&ontology_root).await {
                    eprintln!("❌ [Ontology] Échec de l'initialisation sémantique : {}", e);
                } else {
                    println!("✅ [Ontology] Système sémantique opérationnel (Data-Driven).");
                }
            });

            // 4. GRAPH STORE
            let graph_path = db_root.join("graph_store");
            let storage_for_graph = storage.clone();
            let domain_for_graph = app_config.system_domain.clone();
            let db_for_graph = app_config.system_db.clone();

            let graph_store_result = tauri::async_runtime::block_on(async {
                // 🎯 Instanciation du manager pour le GraphStore
                let manager =
                    CollectionsManager::new(&storage_for_graph, &domain_for_graph, &db_for_graph);
                GraphStore::new(graph_path, &manager).await
            });

            if let Ok(store) = graph_store_result {
                app.manage(store);
                println!("✅ [GraphStore] Base Graphe principale chargée.");
            } else {
                eprintln!("❌ [GraphStore] Echec chargement base graphe.");
            }

            // 5. MIGRATIONS
            let _ = tauri::async_runtime::block_on(run_app_migrations(
                &storage,
                default_space,
                default_db,
            ));

            let plugin_mgr = SharedRef::new(PluginManager::new(&storage, None));

            // 6. INJECTION DES ÉTATS
            app.manage(config);
            let storage_engine = storage.clone();
            app.manage(storage);
            app.manage(plugin_mgr.clone());

            // Instanciation et injection du SessionManager
            let shared_storage = SharedRef::new(storage_engine.clone());
            let session_manager = context::SessionManager::new(shared_storage);
            app.manage(session_manager);

            let app_state = SharedRef::new(AppState {
                model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
            });
            app.manage(app_state.clone());

            app.manage(AsyncMutex::new(WorkflowStore::default()));
            app.manage(AiState::new(None));
            app.manage(DlState::new());

            let app_handle = app.handle();
            raise::blockchain::ensure_innernet_state(app_handle, "default");

            // --- INITIALISATION FABRIC ---
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

            // --- INITIALISATION RÉSEAU ARCADIA ---
            raise::blockchain::p2p::service::init_arcadia_network(app.handle().clone());

            // --- BACKGROUND: IA NATIF ---
            let native_handle = app.handle().clone();
            let storage_for_ia = storage_engine.clone();
            let domain_for_ia = app_config.system_domain.clone();
            let db_for_ia = app_config.system_db.clone();

            tauri::async_runtime::spawn(async move {
                // 🎯 Création du manager pour l'IA et appel asynchrone
                let manager = CollectionsManager::new(&storage_for_ia, &domain_for_ia, &db_for_ia);
                match CandleLlmEngine::new(&manager).await {
                    Ok(engine) => {
                        let state = native_handle.state::<NativeLlmState>();
                        *state.0.lock().unwrap() = Some(engine);
                        println!("✅ [Background] Moteur IA Natif prêt !");
                    }
                    Err(e) => {
                        eprintln!("❌ [Background] Echec chargement IA Natif : {}", e);
                    }
                }
            });

            // --- BACKGROUND: ORCHESTRATEUR IA ET MOTEUR DE WORKFLOW ---
            let app_handle_clone = app.handle().clone();
            let plugin_mgr_for_wf = plugin_mgr.clone();

            tauri::async_runtime::spawn(async move {
                let global_cfg = AppConfig::get();
                let storage_state = app_handle_clone.state::<StorageEngine>();

                let _ = ModelLoader::from_engine(
                    storage_state.inner(),
                    &global_cfg.system_domain,
                    &global_cfg.system_db,
                );

                let storage_state = app_handle_clone.state::<StorageEngine>();
                let loader = ModelLoader::from_engine(storage_state.inner(), "mbse2", "_system");

                if let Ok(model) = loader.load_full_model().await {
                    let storage_arc = SharedRef::new(storage_state.inner().clone());
                    let manager = CollectionsManager::new(
                        &storage_arc,
                        &global_cfg.system_domain,
                        &global_cfg.system_db,
                    );
                    match AiOrchestrator::new(model, &manager, storage_arc.clone()).await {
                        Ok(orchestrator) => {
                            let shared_orch = SharedRef::new(AsyncMutex::new(orchestrator));
                            let ai_state = app_handle_clone.state::<AiState>();
                            *ai_state.0.lock().await = Some(shared_orch.clone());

                            let executor =
                                WorkflowExecutor::new(shared_orch.clone(), plugin_mgr_for_wf);

                            let wf_state = app_handle_clone.state::<AsyncMutex<WorkflowStore>>();
                            let mut wf_store = wf_state.lock().await;
                            wf_store.scheduler = Some(WorkflowScheduler::new(executor));

                            println!(
                                "✅ [RAISE] Orchestrateur IA et Workflow Engine opérationnels."
                            );
                        }
                        Err(e) => eprintln!("❌ Erreur Fatale Orchestrator: {}", e),
                    }
                } else {
                    eprintln!("⚠️ [IA] Impossible de charger le modèle symbolique initial.");
                }
            });

            // --- BOUCLE P2P ---
            let swarm_handle = app.handle().clone();
            let _storage_for_p2p = storage_engine;
            let _app_state_for_p2p = app_state;

            tauri::async_runtime::spawn(async move {
                let _swarm_state =
                    swarm_handle.state::<AsyncMutex<
                        libp2p::Swarm<raise::blockchain::p2p::behavior::ArcadiaBehavior>,
                    >>();
                // ... tout le reste de l'ancienne boucle ...
            });
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
    let migrations = vec![
        Migration {
            id: "init_001_core_collections".to_string(),
            version: "1.0.0".to_string(),
            description: "Init".to_string(),
            up: vec![
                MigrationStep::CreateCollection {
                    name: "articles".to_string(),
                    // 🎯 FIX : On remplace JsonValue::Null par le schéma générique
                    schema: JsonValue::String(
                        "db://_system/_system/schemas/v1/db/generic.schema.json".to_string(),
                    ),
                },
                MigrationStep::CreateCollection {
                    name: "systems".to_string(),
                    // 🎯 FIX
                    schema: JsonValue::String(
                        "db://_system/_system/schemas/v1/db/generic.schema.json".to_string(),
                    ),
                },
                MigrationStep::CreateCollection {
                    name: "exchange_items".to_string(),
                    // 🎯 FIX
                    schema: JsonValue::String(
                        "db://_system/_system/schemas/v1/db/generic.schema.json".to_string(),
                    ),
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
    migrator.run_migrations(migrations).await?;
    Ok(())
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(test)]
    use raise::utils::testing::DbSandbox;

    #[async_test]
    async fn test_load_ontologies_from_directory_success() {
        let dir = tempdir().unwrap();
        let path = dir.path();

        // On simule une structure d'ontologie
        let raise_dir = path.join("raise");
        fs::create_dir_all_sync(&raise_dir).unwrap();

        let content = r#"{ "@context": { "test": "http://test#" }, "@graph": [] }"#;
        fs::write_async(&raise_dir.join("core.jsonld"), content.as_bytes())
            .await
            .unwrap();

        // 🎯 On teste l'initialisation globale
        let res = VocabularyRegistry::init(path).await;
        assert!(res.is_ok());

        let registry = VocabularyRegistry::global();
        assert!(registry.get_default_context().contains_key("test"));
    }

    #[async_test]
    async fn test_migrations_list_integrity() {
        // 1. 🎯 La Sandbox remplace tout le setup manuel en une seule ligne !
        let sandbox = DbSandbox::new().await;

        let space = &sandbox.config.system_domain;
        let db = &sandbox.config.system_db;

        // 2. 🎯 LE CORRECTIF : On force l'initialisation de l'index système
        // Le dossier existe (créé par la Sandbox), mais on doit générer _system.json !
        let manager = CollectionsManager::new(&sandbox.storage, space, db);
        manager
            .init_db()
            .await
            .expect("L'initialisation de l'index système a échoué");

        // 3. Exécution des migrations sur le moteur encapsulé
        let res = run_app_migrations(&sandbox.storage, space, db).await;

        assert!(
            res.is_ok(),
            "Le test de migration a échoué : {:?}",
            res.err()
        );
    }
}
