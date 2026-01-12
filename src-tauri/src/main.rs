// FICHIER : src-tauri/src/main.rs

#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::Mutex;
use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex;

// --- IMPORTS RAISE ---
use raise::ai::training::dataset;
use raise::commands::{
    ai_commands, blockchain_commands, codegen_commands, cognitive_commands, genetics_commands,
    json_db_commands, model_commands, traceability_commands, utils_commands, workflow_commands,
};

// --- IMPORT IA NATIF ---
use raise::ai::llm::candle_engine::CandleLlmEngine;
use raise::ai::llm::NativeLlmState;

use raise::json_db::migrations::migrator::Migrator;
use raise::json_db::migrations::{Migration, MigrationStep};
use raise::json_db::storage::{JsonDbConfig, StorageEngine};
use serde_json::Value;

use raise::plugins::manager::PluginManager;

// Structures d'état
use raise::commands::ai_commands::AiState;
use raise::commands::workflow_commands::WorkflowStore;
use raise::workflow_engine::scheduler::WorkflowScheduler;

pub use raise::model_engine::types::ProjectModel;
use raise::AppState;

use raise::ai::orchestrator::AiOrchestrator;
use raise::graph_store::GraphStore;
use raise::model_engine::loader::ModelLoader;

fn main() {
    // [MODIFICATION] Chargement explicite du .env au démarrage
    // Essentiel pour que l'Orchestrateur (RAG) voie "VECTOR_STORE_PROVIDER" et "ENABLE_GRAPH_VECTORS"
    dotenvy::dotenv().ok();

    println!("🚀 Démarrage de RAISE...");
    raise::utils::init_logging();
    let _ = raise::utils::AppConfig::init();

    tauri::Builder::default()
        // 0. GESTION ETAT IA NATIF
        .manage(NativeLlmState(std::sync::Mutex::new(None)))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // 1. CONFIG DOMAINE
            let db_root = if let Ok(env_path) = env::var("PATH_RAISE_DOMAIN") {
                PathBuf::from(env_path)
            } else {
                app.path().app_data_dir().unwrap().join("raise_db")
            };
            if !db_root.exists() {
                fs::create_dir_all(&db_root)?;
            }

            // 2. CONFIG STORAGE
            let config = JsonDbConfig::new(db_root.clone());
            let storage = StorageEngine::new(config.clone());
            let default_space = "un2";
            let default_db = "_system";

            // 3. GRAPH STORE (Base Graphe globale pour l'app)
            // Note : Le RAG utilise son propre GraphStore interne dans un sous-dossier différent (via RagRetriever)
            // pour éviter les conflits de verrouillage de fichiers (RocksDB).
            let graph_path = db_root.join("graph_store");
            let graph_store_result =
                tauri::async_runtime::block_on(async { GraphStore::new(graph_path).await });

            if let Ok(store) = graph_store_result {
                app.manage(store);
                println!("✅ [GraphStore] Base Graphe principale chargée.");
            } else {
                eprintln!("❌ [GraphStore] Echec chargement base graphe.");
            }

            // 4. MIGRATIONS
            let _ = run_app_migrations(&storage, default_space, default_db);
            let plugin_mgr = PluginManager::new(&storage);

            // 5. INJECTION ÉTATS
            app.manage(config);
            app.manage(storage);
            app.manage(plugin_mgr);
            app.manage(AppState {
                model: Mutex::new(ProjectModel::default()),
            });

            // Initialisation "vide" pour le démarrage
            app.manage(AsyncMutex::new(WorkflowStore::default()));
            app.manage(AiState::new(None));

            let app_handle = app.handle();
            raise::blockchain::ensure_innernet_state(app_handle, "default");

            // --- CHARGEMENT IA NATIF (PATRIMOINE CONSERVÉ) ---
            let native_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                println!("⏳ [Background] Initialisation du moteur IA Natif (Llama 3.2 1B)...");
                match CandleLlmEngine::new() {
                    Ok(engine) => {
                        let state = native_handle.state::<NativeLlmState>();
                        // CORRECTION : Utilisation de unwrap() au lieu de if let pour éviter lifetime issue
                        *state.0.lock().unwrap() = Some(engine);
                        println!("✅ [Background] Moteur IA Natif prêt !");
                    }
                    Err(e) => {
                        eprintln!("❌ [Background] Echec chargement IA Natif : {}", e);
                    }
                }
            });

            // 6. INITIALISATION ASYNC ORCHESTRATEUR
            // C'est ici que l'on connecte notre nouvel Orchestrateur optimisé
            let app_handle_clone = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                // [MODIFICATION] Récupération robuste des URLs
                let llm_url =
                    env::var("RAISE_LOCAL_URL").unwrap_or_else(|_| "http://127.0.0.1:8081".into());

                // Le port Qdrant est optionnel si on est en mode "surreal", mais on le garde pour la compatibilité
                let qdrant_url = format!(
                    "http://127.0.0.1:{}",
                    env::var("PORT_QDRANT_GRPC").unwrap_or_else(|_| "6334".into())
                );

                println!(
                    "🤖 [IA] Chargement Orchestrateur (RAG/LLM) sur {}...",
                    llm_url
                );

                let storage_state = app_handle_clone.state::<StorageEngine>();

                // Chargement du modèle symbolique (Arcadia) depuis le JSON DB
                let loader = ModelLoader::from_engine(storage_state.inner(), "un2", "_system");
                let model_res = loader.load_full_model();

                match model_res {
                    Ok(model) => {
                        println!("✅ [IA] Modèle symbolique chargé. Démarrage Orchestrateur...");

                        // [MODIFICATION] Appel correspondant à la nouvelle signature de AiOrchestrator::new
                        // L'orchestrateur va lire VECTOR_STORE_PROVIDER dans l'env pour choisir entre Qdrant/Surreal
                        match AiOrchestrator::new(model, &qdrant_url, &llm_url).await {
                            Ok(orchestrator) => {
                                // On enveloppe dans un Arc<Mutex> pour le partage thread-safe
                                let shared_orch = Arc::new(AsyncMutex::new(orchestrator));

                                // Injection dans le State Tauri (pour ai_commands)
                                let ai_state = app_handle_clone.state::<AiState>();
                                let mut guard = ai_state.0.lock().await;
                                *guard = Some(shared_orch.clone());

                                // Injection dans le Scheduler (Workflow)
                                let wf_state =
                                    app_handle_clone.state::<AsyncMutex<WorkflowStore>>();
                                let mut wf_store = wf_state.lock().await;
                                wf_store.scheduler = Some(WorkflowScheduler::new(shared_orch));

                                println!("✅ [RAISE] Orchestrateur IA opérationnel (Hybride).");
                            }
                            Err(e) => eprintln!("❌ Erreur Fatale Orchestrator: {}", e),
                        }
                    }
                    Err(e) => eprintln!("❌ Erreur Chargement Modèle Projet : {}", e),
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Commandes JSON DB
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
            // Commandes Modèle
            model_commands::load_project_model,
            // Commandes IA (Utilisant l'Orchestrateur via AiState)
            ai_commands::ai_chat,
            ai_commands::ai_reset,
            ai_commands::ask_native_llm,
            ai_commands::ai_learn_text,
            dataset::ai_export_dataset,
            // Commandes Cognitives (Plugins)
            cognitive_commands::cognitive_load_plugin,
            cognitive_commands::cognitive_run_plugin,
            cognitive_commands::cognitive_list_plugins,
            // Commandes Blockchain
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
            // Commandes Métier
            genetics_commands::run_genetic_optimization,
            codegen_commands::generate_source_code,
            traceability_commands::analyze_impact,
            traceability_commands::run_compliance_audit,
            traceability_commands::get_traceability_matrix,
            traceability_commands::get_element_neighbors,
            utils_commands::get_app_info,
            // Commandes Workflow
            workflow_commands::submit_mandate,
            workflow_commands::register_workflow,
            workflow_commands::start_workflow,
            workflow_commands::resume_workflow,
            workflow_commands::get_workflow_state,
            workflow_commands::set_sensor_value
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

fn run_app_migrations(storage: &StorageEngine, space: &str, db: &str) -> anyhow::Result<()> {
    let migrator = Migrator::new(storage, space, db);
    let migrations = vec![
        Migration {
            id: "init_001_core_collections".to_string(),
            version: "1.0.0".to_string(),
            description: "Init".to_string(),
            up: vec![
                MigrationStep::CreateCollection {
                    name: "articles".to_string(),
                    schema: Value::Null,
                },
                MigrationStep::CreateCollection {
                    name: "systems".to_string(),
                    schema: Value::Null,
                },
                MigrationStep::CreateCollection {
                    name: "exchange_items".to_string(),
                    schema: Value::Null,
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
    migrator.run_migrations(migrations)?;
    Ok(())
}
