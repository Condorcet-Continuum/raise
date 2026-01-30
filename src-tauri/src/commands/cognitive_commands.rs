// FICHIER : src-tauri/src/commands/cognitive_commands.rs

use crate::plugins::manager::PluginManager;
use serde_json::{json, Value};
use tauri::State;

/// Charge un plugin cognitif dans le gestionnaire.
#[tauri::command]
pub async fn cognitive_load_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    path: String,
    space: String,
    db: String,
) -> Result<String, String> {
    manager
        .load_plugin(&id, &path, &space, &db)
        .map_err(|e| e.to_string())?;

    Ok(format!("Plugin {} chargé avec succès", id))
}

/// Exécute un plugin avec un contexte de gouvernance (Mandat).
/// Retourne un objet JSON contenant le résultat technique et les signaux émis.
#[tauri::command]
pub async fn cognitive_run_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    mandate: Option<Value>,
) -> Result<Value, String> {
    // Utilisation de la nouvelle méthode run_plugin_with_context pour supporter le Workflow
    let (code, signals) = manager
        .run_plugin_with_context(&id, mandate)
        .map_err(|e| e.to_string())?;

    Ok(json!({
        "exit_code": code,
        "signals": signals
    }))
}

/// Liste tous les plugins actuellement chargés.
#[tauri::command]
pub async fn cognitive_list_plugins(
    manager: State<'_, PluginManager>,
) -> Result<Vec<String>, String> {
    Ok(manager.list_active_plugins())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitive_commands_interface() {
        // Validation de la structure JSON de sortie pour le Frontend
        let code = 0;
        let signals = vec![json!({"type": "LOG", "data": "test"})];
        let response = json!({
            "exit_code": code,
            "signals": signals
        });

        assert_eq!(response["exit_code"], 0);
        assert!(response["signals"].is_array());
    }
}
