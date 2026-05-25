// FICHIER : crates/raise-desktop/src/commands/genetics_commands.rs

use raise_core::genetics::dto::{OptimizationRequest, OptimizationResult};
use raise_core::utils::prelude::*;

// 🎯 On importe le service métier
use raise_core::services::genetics_service;

use tauri::{command, AppHandle, Emitter};

#[command]
pub fn debug_genetics_ping(name: String) -> String {
    genetics_service::debug_genetics_ping(name)
}

#[command]
pub async fn run_architecture_optimization(
    app: AppHandle,
    params: OptimizationRequest,
) -> RaiseResult<OptimizationResult> {
    // Tauri transmet une closure (callback) au noyau pour émettre la progression vers l'UI
    genetics_service::run_architecture_optimization(params, move |progress| {
        let _ = app.emit("genetics-progress", progress);
    })
    .await
}
