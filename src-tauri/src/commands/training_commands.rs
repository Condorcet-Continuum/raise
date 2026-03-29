// FICHIER : src-tauri/src/commands/training_commands.rs

use crate::ai::training::ai_train_domain_native;
use crate::json_db::collections::manager::CollectionsManager; // 🎯 Import du manager
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
    // 🎯 FIX : On instancie le manager pour la base demandée
    let manager = CollectionsManager::new(storage.inner(), &space, &db_name);

    // Et on le passe au moteur d'entraînement
    ai_train_domain_native(&manager, &domain, epochs, lr).await
}
