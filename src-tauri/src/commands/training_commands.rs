// FICHIER : src-tauri/src/commands/training_commands.rs

use crate::ai::training::ai_train_domain_native;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

#[tauri::command]
pub async fn tauri_train_domain(
    storage: tauri::State<'_, StorageEngine>, // Injection de dépendance Tauri
    space: String,
    db_name: String,
    domain: String,
    epochs: usize,
    lr: f64,
) -> RaiseResult<String> {
    // On appelle simplement la fonction pure du noyau !
    ai_train_domain_native(storage.inner(), &space, &db_name, &domain, epochs, lr).await
}
