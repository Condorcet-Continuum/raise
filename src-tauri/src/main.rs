// src-tauri/src/main.rs
// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use tauri::Manager;

// CORRECTION : On n'utilise PAS 'mod' ici.
// On importe les modules depuis la librairie 'genaptitude' (définie dans lib.rs)
use genaptitude::commands::{blockchain_commands, json_db_commands, model_commands};
use genaptitude::json_db::storage::{JsonDbConfig, StorageEngine};

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_fs::init())
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .expect("failed to get app data dir");
            let db_root = app_data_dir.join("genaptitude_db");

            // Création de la config
            let config = JsonDbConfig::new(db_root);

            // Création et injection du moteur
            let storage = StorageEngine::new(config);
            app.manage(storage);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Commandes JSON-DB
            json_db_commands::jsondb_create_collection,
            json_db_commands::jsondb_list_collections,
            json_db_commands::jsondb_insert_document,
            json_db_commands::jsondb_get_document,
            json_db_commands::jsondb_update_document,
            json_db_commands::jsondb_delete_document,
            json_db_commands::jsondb_execute_query,
            json_db_commands::jsondb_execute_sql,
            json_db_commands::jsondb_list_all,
            // Commandes Modèle
            model_commands::load_project_model,
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
            blockchain_commands::vpn_check_installation
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
