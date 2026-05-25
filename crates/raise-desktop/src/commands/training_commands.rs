// FICHIER : crates/raise-desktop/src/commands/training_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*;

// 🎯 On importe le service
use raise_core::services::training_service;

#[tauri::command]
pub async fn tauri_train_domain(
    storage: tauri::State<'_, StorageEngine>,
    space: String,
    db_name: String,
    domain: String,
    epochs: usize,
    lr: f64,
) -> RaiseResult<String> {
    training_service::train_domain(storage.inner(), &space, &db_name, &domain, epochs, lr).await
}
