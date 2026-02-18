// FICHIER : src-tauri/src/commands/cognitive_commands.rs
use crate::utils::{data::Value, prelude::*};

use crate::plugins::manager::PluginManager;
use tauri::State;

/// Charge un plugin cognitif dans le gestionnaire.
#[tauri::command]
pub async fn cognitive_load_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    path: String,
    space: String,
    db: String,
) -> Result<String> {
    manager
        .load_plugin(&id, &path, &space, &db)
        .await
        .map_err(|e| AppError::from(e.to_string()))?;

    Ok(format!("Plugin {} chargé avec succès", id))
}

/// Exécute un plugin avec un contexte de gouvernance (Mandat).
/// Retourne un objet JSON contenant le résultat technique et les signaux émis.
#[tauri::command]
pub async fn cognitive_run_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    mandate: Option<Value>,
) -> Result<Value> {
    // Utilisation de la nouvelle méthode run_plugin_with_context pour supporter le Workflow
    let (code, signals) = manager
        .run_plugin_with_context(&id, mandate)
        .await
        .map_err(|e| AppError::from(e.to_string()))?;

    Ok(json!({
        "exit_code": code,
        "signals": signals
    }))
}

/// Liste tous les plugins actuellement chargés.
#[tauri::command]
pub async fn cognitive_list_plugins(manager: State<'_, PluginManager>) -> Result<Vec<String>> {
    Ok(manager.list_active_plugins().await)
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
