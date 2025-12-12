// FICHIER : src-tauri/src/main.rs
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;
use std::path::PathBuf;
use tauri::Manager;

use genaptitude::commands::{
    ai_commands, blockchain_commands, codegen_commands, cognitive_commands, genetics_commands,
    json_db_commands, model_commands, traceability_commands,
};
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};

fn main() {
    dotenvy::dotenv().ok();
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
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
            app.manage(storage);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // --- GESTION DATABASE (NOUVEAU) ---
            json_db_commands::jsondb_create_db,
            json_db_commands::jsondb_drop_db,
            // --- GESTION COLLECTIONS ---
            json_db_commands::jsondb_create_collection,
            json_db_commands::jsondb_list_collections,
            json_db_commands::jsondb_drop_collection, // (Ajouté)
            // --- GESTION INDEXES (NOUVEAU) ---
            json_db_commands::jsondb_create_index,
            json_db_commands::jsondb_drop_index,
            // --- CRUD DOCUMENTS ---
            json_db_commands::jsondb_insert_document,
            json_db_commands::jsondb_get_document,
            json_db_commands::jsondb_update_document,
            json_db_commands::jsondb_delete_document,
            json_db_commands::jsondb_list_all,
            // --- REQUÊTES ---
            json_db_commands::jsondb_execute_query,
            json_db_commands::jsondb_execute_sql,
            // --- AUTRES COMMANDES EXISTANTES ---
            model_commands::load_project_model,
            ai_commands::ai_chat,
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
            // --- OPTIMISATION GÉNÉTIQUE ---
            genetics_commands::run_genetic_optimization,
            // --- GÉNÉRATEUR DE CODE ---
            codegen_commands::generate_source_code,
            // BLOCS COGNITIFS (WASM) ---
            cognitive_commands::run_consistency_analysis,
            // --- TRAÇABILITÉ & AUDIT (NOUVEAU) ---
            traceability_commands::analyze_impact,
            traceability_commands::run_compliance_audit,
            traceability_commands::get_traceability_matrix,
            traceability_commands::get_element_neighbors
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
