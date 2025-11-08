#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{fs, path::PathBuf};
use tauri::{command, AppHandle, Builder, Manager};

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

fn main() {
    Builder::default()
        .invoke_handler(tauri::generate_handler![register_schema, get_schema])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
