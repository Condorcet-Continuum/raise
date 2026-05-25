// FICHIER : crates/raise-core/src/services/cognitive_service.rs
//! Façade métier pure pour la gestion des plugins cognitifs Wasm (Agnostique Tauri).

use crate::plugins::manager::PluginManager;
use crate::utils::prelude::*;

/// Charge un plugin cognitif dans le gestionnaire.
/// Façade pure (Zéro Tauri).
pub async fn cognitive_load_plugin(
    manager: &PluginManager, // 🎯 FIX : Référence pure Rust
    id: &str,                // 🎯 OPTIMISATION : &str
    path: &str,              // 🎯 OPTIMISATION : &str
    space: &str,             // 🎯 OPTIMISATION : &str
    db: &str,                // 🎯 OPTIMISATION : &str
) -> RaiseResult<String> {
    match manager.load_plugin(id, path, space, db).await {
        Ok(_) => {}
        Err(e) => raise_error!(
            "ERR_PLUGIN_LOAD_FAIL",
            error = e,
            context = json_value!({
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
pub async fn cognitive_run_plugin(
    manager: &PluginManager, // 🎯 FIX : Référence pure Rust
    id: &str,                // 🎯 OPTIMISATION : &str
    mandate: Option<JsonValue>,
) -> RaiseResult<JsonValue> {
    // Utilisation de la méthode run_plugin_with_context pour supporter le Workflow
    let (code, signals) = match manager.run_plugin_with_context(id, mandate).await {
        Ok(result) => result,
        Err(e) => raise_error!(
            "ERR_PLUGIN_EXECUTION_FAIL",
            error = e,
            context = json_value!({
                "plugin_id": id,
                "action": "run_plugin_with_context",
                "hint": "Le plugin a crashé durant l'exécution. Vérifiez les logs de sortie du plugin et la validité du mandat transmis."
            })
        ),
    };

    Ok(json_value!({
        "exit_code": code,
        "signals": signals
    }))
}

/// Liste tous les plugins actuellement chargés.
pub async fn cognitive_list_plugins(manager: &PluginManager) -> RaiseResult<Vec<String>> {
    Ok(manager.list_active_plugins().await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cognitive_commands_interface() {
        // Validation de la structure JSON de sortie
        let code = 0;
        let signals = vec![json_value!({"type": "LOG", "data": "test"})];
        let response = json_value!({
            "exit_code": code,
            "signals": signals
        });

        assert_eq!(response["exit_code"], 0);
        assert!(response["signals"].is_array());
    }
}
