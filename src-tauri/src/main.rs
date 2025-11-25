#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use genaptitude::commands::blockchain_commands;
use genaptitude::commands::json_db_commands;

use std::{fs, path::PathBuf};
use tauri::{command, AppHandle, Builder, Manager};
use tracing_subscriber::{fmt, EnvFilter}; // <- on utilise la lib genaptitude

fn ensure_schema_dir(app: &AppHandle) -> Result<PathBuf, String> {
    // Dossier de données de l'app (ex: ~/.local/share/GenAptitude/schemas)
    let mut dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("app_data_dir unavailable: {e}"))?;
    dir.push("schemas");
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

#[command]
fn register_schema(app: AppHandle, schema_id: String, schema_json: String) -> Result<(), String> {
    let dir = ensure_schema_dir(&app)?;
    let file = dir.join(format!("{schema_id}.json"));
    fs::write(file, schema_json).map_err(|e| e.to_string())
}

#[command]
fn get_schema(app: AppHandle, schema_id: String) -> Result<String, String> {
    let dir = ensure_schema_dir(&app)?;
    let file = dir.join(format!("{schema_id}.json"));
    fs::read_to_string(file).map_err(|e| e.to_string())
}
fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    fmt().with_env_filter(filter).init();
}

fn main() {
    init_tracing();

    Builder::default()
        .plugin(tauri_plugin_fs::init())
        .invoke_handler(tauri::generate_handler![
            // 🔹 tes commandes existantes
            register_schema,
            get_schema,
            // 🔹 commandes JSON-DB (définies dans json_db_commands.rs)
            json_db_commands::jsondb_create_collection,
            json_db_commands::jsondb_drop_collection,
            json_db_commands::jsondb_insert_with_schema,
            json_db_commands::jsondb_upsert_with_schema,
            json_db_commands::jsondb_insert_raw,
            json_db_commands::jsondb_update_with_schema,
            json_db_commands::jsondb_update_raw,
            json_db_commands::jsondb_get,
            json_db_commands::jsondb_delete,
            json_db_commands::jsondb_list_ids,
            json_db_commands::jsondb_list_all,
            json_db_commands::jsondb_refresh_registry,
            json_db_commands::jsondb_query_collection, // 🆕 avec QueryEngine
            json_db_commands::jsondb_insert,
            json_db_commands::jsondb_upsert,
            json_db_commands::jsondb_list_collections,
            json_db_commands::jsondb_query_collection,
            json_db_commands::jsondb_execute_transaction,
            // 🔗 Blockchain / VPN (module blockchain_commands)
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
