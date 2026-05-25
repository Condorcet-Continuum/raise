// FICHIER : crates/raise-desktop/src/commands/model_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::model_engine::types::ProjectModel;
use raise_core::services::model_service;
use raise_core::utils::prelude::*;

use tauri::{command, State};

#[command]
pub async fn load_project_model(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<ProjectModel> {
    model_service::load_project_model(storage.inner(), &space, &db).await
}
