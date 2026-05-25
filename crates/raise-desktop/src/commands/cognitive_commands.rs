// FICHIER : crates/raise-desktop/src/commands/cognitive_commands.rs

use raise_core::plugins::manager::PluginManager;
use raise_core::utils::prelude::*;

// 🎯 On importe le service métier du noyau
use raise_core::services::cognitive_service;

use tauri::{command, State};

#[command]
pub async fn cognitive_load_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    path: String,
    space: String,
    db: String,
) -> RaiseResult<String> {
    cognitive_service::cognitive_load_plugin(manager.inner(), &id, &path, &space, &db).await
}

#[command]
pub async fn cognitive_run_plugin(
    manager: State<'_, PluginManager>,
    id: String,
    mandate: Option<JsonValue>,
) -> RaiseResult<JsonValue> {
    cognitive_service::cognitive_run_plugin(manager.inner(), &id, mandate).await
}

#[command]
pub async fn cognitive_list_plugins(manager: State<'_, PluginManager>) -> RaiseResult<Vec<String>> {
    cognitive_service::cognitive_list_plugins(manager.inner()).await
}
