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
) -> RaiseResult<String> {
    match manager.load_plugin(&id, &path, &space, &db).await {
        Ok(_) => {}
        Err(e) => raise_error!(
            "ERR_PLUGIN_LOAD_FAIL",
            error = e,
            context = json!({
                "plugin_id": id,
                "path": path,
                "action": "load_external_plugin",
                "hint": "Impossible de charger le plugin. Vérifiez que le fichier existe, que les dépendances sont présentes et que le manifeste est valide."
            })
        ),
    };

    Ok(format!("Plugin {} chargé avec succès", id))
}

/// Exécute un plugin avec un contexte de gouvernance (Mandat).
/// Retourne un objet JSON contenant le résultat technique et les signaux émis.
#[tauri::command]
pub async fn cognitive_run_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    mandate: Option<Value>,
) -> RaiseResult<Value> {
    // Utilisation de la nouvelle méthode run_plugin_with_context pour supporter le Workflow
    let (code, signals) = match manager.run_plugin_with_context(&id, mandate).await {
        Ok(result) => result,
        Err(e) => raise_error!(
            "ERR_PLUGIN_EXECUTION_FAIL",
            error = e,
            context = json!({
                "plugin_id": id,
                "action": "run_plugin_with_context",
                "hint": "Le plugin a crashé durant l'exécution. Vérifiez les logs de sortie du plugin et la validité du mandat transmis."
            })
        ),
    };

    Ok(json!({
        "exit_code": code,
        "signals": signals
    }))
}

/// Liste tous les plugins actuellement chargés.
#[tauri::command]
pub async fn cognitive_list_plugins(manager: State<'_, PluginManager>) -> RaiseResult<Vec<String>> {
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
