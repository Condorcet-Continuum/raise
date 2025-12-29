// FICHIER : src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::path::PathBuf;
use std::sync::Mutex; // Mutex Standard (Synchronous) pour AppState
use tauri::Manager;
// 1. On donne un alias explicite au Mutex Async
use tokio::sync::Mutex as AsyncMutex;

use genaptitude::ai::training;
use genaptitude::commands::{
    ai_commands, blockchain_commands, codegen_commands, cognitive_commands, genetics_commands,
    json_db_commands, model_commands, traceability_commands, utils_commands, workflow_commands,
};
// Import du State IA
use genaptitude::commands::ai_commands::AiState;
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};

// Import des structures d'état
use genaptitude::commands::workflow_commands::WorkflowStore;
use genaptitude::model_engine::types::ProjectModel;
use genaptitude::AppState; // Contient std::sync::Mutex<ProjectModel>

// Imports pour l'initialisation Background de l'IA
use genaptitude::ai::orchestrator::AiOrchestrator;
use genaptitude::model_engine::loader::ModelLoader;

fn main() {
    // Initialisation des logs & config via utils
    genaptitude::utils::init_logging();
    let _ = genaptitude::utils::AppConfig::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // --- 1. CONFIGURATION DB ---
            let db_root = if let Ok(env_path) = env::var("PATH_GENAPTITUDE_DOMAIN") {
                println!("📂 Utilisation de la DB personnalisée : {}", env_path);
                PathBuf::from(env_path)
            } else {
                let app_data_dir = app
                    .path()
                    .app_data_dir()
                    .expect("failed to get app data dir");
                let path = app_data_dir.join("genaptitude_db");
                println!("📂 Utilisation de la DB système par défaut : {:?}", path);
                path
            };

            let config = JsonDbConfig::new(db_root);
            let storage = StorageEngine::new(config);

            // Enregistrement du StorageEngine
            app.manage(storage);

            // --- 2. CONFIGURATION ÉTATS GLOBAUX ---

            // A. AppState (Modèle Arcadia pour l'UI Traceability)
            app.manage(AppState {
                model: Mutex::new(ProjectModel::default()),
            });

            // B. WorkflowStore
            app.manage(AsyncMutex::new(WorkflowStore::default()));

            // C. Blockchain
            let app_handle = app.handle();
            genaptitude::blockchain::ensure_innernet_state(app_handle, "default");

            // --- 3. INITIALISATION IA (BACKGROUND) ---

            // D. On enregistre l'état IA vide (Mutex Async)
            app.manage(AiState::new(None));

            let app_handle_clone = app.handle().clone();

            // E. Lancement du chargement asynchrone
            tauri::async_runtime::spawn(async move {
                // Récupération des variables d'env (chargées par dotenv précédemment dans utils::AppConfig)
                let llm_url = env::var("GENAPTITUDE_LOCAL_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
                let qdrant_port =
                    env::var("PORT_QDRANT_GRPC").unwrap_or_else(|_| "6334".to_string());
                let qdrant_url = format!("http://127.0.0.1:{}", qdrant_port);

                println!("🤖 [IA] Démarrage du processus d'initialisation...");

                // On récupère le StorageEngine déjà managé
                // Note : .state() retourne un State<T>, on utilise inner() pour accéder à l'objet réel
                let storage_state = app_handle_clone.state::<StorageEngine>();
                // On clone le moteur pour qu'il soit 'Send' vers le thread bloquant
                let storage_engine = storage_state.inner().clone();

                // Chargement du modèle (Lourd -> spawn_blocking)
                // TODO: Rendre "un2" et "_system" configurables via .env ou UI
                let model_res = tauri::async_runtime::spawn_blocking(move || {
                    let loader = ModelLoader::from_engine(&storage_engine, "un2", "_system");
                    loader.load_full_model()
                })
                .await;

                match model_res {
                    Ok(Ok(model)) => {
                        println!("🤖 [IA] Modèle chargé. Connexion à Qdrant & LLM...");
                        match AiOrchestrator::new(model, &qdrant_url, &llm_url).await {
                            Ok(orchestrator) => {
                                let ai_state = app_handle_clone.state::<AiState>();
                                let mut guard = ai_state.lock().await;
                                *guard = Some(orchestrator);
                                println!("✅ [IA] GenAptitude est PRÊTE (Mémoire + RAG + Modèle)");
                            }
                            Err(e) => eprintln!("❌ [IA] Erreur Connexion Orchestrator : {}", e),
                        }
                    }
                    Ok(Err(e)) => eprintln!("❌ [IA] Erreur Chargement Modèle JSON-DB : {}", e),
                    Err(e) => eprintln!("❌ [IA] Erreur Thread Panicked : {}", e),
                }
            });

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- GESTION DATABASE ---
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
            // <-- MOTEUR DE RÈGLES --->
            json_db_commands::jsondb_evaluate_draft,
            json_db_commands::jsondb_init_demo_rules,
            // --- MODEL & ARCHITECTURE ---
            model_commands::load_project_model,
            // --- IA & AGENTS ---
            ai_commands::ai_chat,
            ai_commands::ai_reset,
            training::dataset::ai_export_dataset,
            // --- BLOCKCHAIN ---
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
            // --- OPTIMISATION ---
            genetics_commands::run_genetic_optimization,
            // --- CODEGEN ---
            codegen_commands::generate_source_code,
            // --- COGNITIVE (WASM) ---
            cognitive_commands::run_consistency_analysis,
            // --- TRAÇABILITÉ ---
            traceability_commands::analyze_impact,
            traceability_commands::run_compliance_audit,
            traceability_commands::get_traceability_matrix,
            traceability_commands::get_element_neighbors,
            // --- UTILITAIRES ---
            utils_commands::get_app_info,
            // --- WORKFLOW ENGINE ---
            workflow_commands::register_workflow,
            workflow_commands::start_workflow,
            workflow_commands::resume_workflow,
            workflow_commands::get_workflow_state
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
