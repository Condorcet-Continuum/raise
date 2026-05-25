// FICHIER : src-tauri/src/services/training_service.rs

use crate::ai::training::ai_train_domain_native;
use crate::json_db::collections::manager::CollectionsManager; // 🎯 Import du manager
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;

pub async fn train_domain(
    storage: &StorageEngine, // Injection de dépendance Tauri
    space: &str,
    db_name: &str,
    domain: &str,
    epochs: usize,
    lr: f64,
) -> RaiseResult<String> {
    // 🎯 FIX : On instancie le manager pour la base demandée
    let manager = CollectionsManager::new(storage, space, db_name);

    // Et on le passe au moteur d'entraînement
    ai_train_domain_native(&manager, domain, epochs, lr).await
}
