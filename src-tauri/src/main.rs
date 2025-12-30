// FICHIER : src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex; // Mutex Standard pour AppState
use tauri::Manager;
use tokio::sync::Mutex as AsyncMutex; // Mutex Async pour l'IA et Workflow

// --- IMPORTS GENAPTITUDE ---
use genaptitude::ai::training; // Pour dataset import/export
use genaptitude::commands::{
    ai_commands, blockchain_commands, codegen_commands, cognitive_commands, genetics_commands,
    json_db_commands, model_commands, traceability_commands, utils_commands, workflow_commands,
};

// Architecture JSON-DB & Plugins
use genaptitude::json_db::migrations::migrator::Migrator;
use genaptitude::json_db::migrations::{Migration, MigrationStep};
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};
use serde_json::Value; // Pour Value::Null

use genaptitude::plugins::manager::PluginManager;

// Structures d'état
use genaptitude::commands::ai_commands::AiState;
use genaptitude::commands::workflow_commands::WorkflowStore;
use genaptitude::model_engine::types::ProjectModel;
use genaptitude::AppState;

// Imports pour l'initialisation Background de l'IA
use genaptitude::ai::orchestrator::AiOrchestrator;
use genaptitude::model_engine::loader::ModelLoader;

fn main() {
    // 1. Initialisation des logs & de la configuration globale
    genaptitude::utils::init_logging();
    let _ = genaptitude::utils::AppConfig::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            // =================================================================
            // 2. CONFIGURATION DU STOCKAGE (DB)
            // =================================================================

            let db_root = if let Ok(env_path) = env::var("PATH_GENAPTITUDE_DOMAIN") {
                if env_path.starts_with("~/") {
                    let home = env::var("HOME").expect("Impossible de trouver la variable $HOME");
                    let expanded = env_path.replace("~", &home);
                    println!(
                        "📂 Configuration DB Personnalisée (Expanded) : {}",
                        expanded
                    );
                    PathBuf::from(expanded)
                } else {
                    println!("📂 Configuration DB Personnalisée : {}", env_path);
                    PathBuf::from(env_path)
                }
            } else {
                let default_path = app.path().app_data_dir().unwrap().join("genaptitude_db");
                println!("📂 Configuration DB Par défaut : {:?}", default_path);
                default_path
            };

            let config = JsonDbConfig::new(db_root);
            let storage = StorageEngine::new(config.clone());

            let default_space = "un2";
            let default_db = "default";

            // =================================================================
            // 3. MIGRATIONS AUTOMATIQUES
            // =================================================================
            println!("⚙️ Vérification des migrations au démarrage...");
            if let Err(e) = run_app_migrations(&storage, default_space, default_db) {
                eprintln!("❌ ERREUR CRITIQUE MIGRATIONS : {}", e);
            } else {
                println!("✅ Migrations : Base de données à jour.");
            }

            // =================================================================
            // 4. INITIALISATION DES PLUGINS COGNITIFS
            // =================================================================
            let plugin_mgr = PluginManager::new(&storage);

            // Auto-chargement des plugins .wasm
            let plugins_dir = app.path().app_data_dir().unwrap().join("plugins");
            if plugins_dir.exists() {
                if let Ok(entries) = fs::read_dir(plugins_dir) {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().and_then(|s| s.to_str()) == Some("wasm") {
                            let name = path.file_stem().unwrap().to_string_lossy().to_string();
                            println!("🔌 Plugin détecté : {} -> Chargement...", name);
                            if let Err(e) = plugin_mgr.load_plugin(
                                &name,
                                path.to_str().unwrap(),
                                default_space,
                                default_db,
                            ) {
                                eprintln!("⚠️ Échec chargement plugin '{}': {}", name, e);
                            }
                        }
                    }
                }
            } else {
                let _ = fs::create_dir_all(&plugins_dir);
            }

            // =================================================================
            // 5. INJECTION DES ÉTATS
            // =================================================================
            app.manage(config);
            app.manage(storage);
            app.manage(plugin_mgr);

            app.manage(AppState {
                model: Mutex::new(ProjectModel::default()),
            });
            app.manage(AsyncMutex::new(WorkflowStore::default()));
            app.manage(AiState::new(None));

            let app_handle = app.handle();
            genaptitude::blockchain::ensure_innernet_state(app_handle, "default");

            // =================================================================
            // 6. INITIALISATION IA (BACKGROUND)
            // =================================================================
            let app_handle_clone = app.handle().clone();

            tauri::async_runtime::spawn(async move {
                let llm_url = env::var("GENAPTITUDE_LOCAL_URL")
                    .unwrap_or_else(|_| "http://127.0.0.1:8081".to_string());
                let qdrant_port =
                    env::var("PORT_QDRANT_GRPC").unwrap_or_else(|_| "6334".to_string());
                let qdrant_url = format!("http://127.0.0.1:{}", qdrant_port);

                println!("🤖 [IA] Démarrage du processus d'initialisation...");

                let storage_state = app_handle_clone.state::<StorageEngine>();
                let storage_engine = storage_state.inner().clone();

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
                                println!("✅ [IA] GenAptitude est PRÊTE.");
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
            // --- IA & DATASETS ---
            ai_commands::ai_chat,
            ai_commands::ai_reset,
            training::dataset::ai_export_dataset,
            // ⚠️ COMMANDE IMPORT DÉSACTIVÉE TANT QUE NON IMPLÉMENTÉE DANS dataset.rs
            // training::dataset::ai_import_dataset,

            // --- PLUGINS COGNITIFS ---
            cognitive_commands::cognitive_load_plugin,
            cognitive_commands::cognitive_run_plugin,
            cognitive_commands::cognitive_list_plugins,
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

fn run_app_migrations(storage: &StorageEngine, space: &str, db: &str) -> anyhow::Result<()> {
    let migrator = Migrator::new(storage, space, db);

    let migrations = vec![
        Migration {
            id: "init_001_core_collections".to_string(),
            version: "1.0.0".to_string(),
            description: "Création des collections de base Arcadia".to_string(),
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
            description: "Indexation des articles par titre".to_string(),
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
