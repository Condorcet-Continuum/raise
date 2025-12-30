use crate::plugins::manager::PluginManager;
use tauri::State;

#[tauri::command]
pub async fn cognitive_load_plugin(
    manager: State<'_, PluginManager>,
    plugin_id: String,
    wasm_path: String,
    space: Option<String>,
    db: Option<String>,
) -> Result<(), String> {
    // Valeurs par défaut si non fournies
    let space = space.unwrap_or_else(|| "un2".to_string());
    let db = db.unwrap_or_else(|| "default".to_string());

    manager
        .load_plugin(&plugin_id, &wasm_path, &space, &db)
        .map_err(|e| format!("Erreur chargement plugin : {}", e))?;

    Ok(())
}

#[tauri::command]
pub async fn cognitive_run_plugin(
    manager: State<'_, PluginManager>,
    plugin_id: String,
) -> Result<i32, String> {
    println!("🦀 Commande : Exécution du plugin '{}'", plugin_id);

    let result = manager
        .run_plugin(&plugin_id)
        .map_err(|e| format!("Erreur exécution plugin : {}", e))?;

    Ok(result)
}

#[tauri::command]
pub async fn cognitive_list_plugins(
    manager: State<'_, PluginManager>,
) -> Result<Vec<String>, String> {
    Ok(manager.list_active_plugins())
}
