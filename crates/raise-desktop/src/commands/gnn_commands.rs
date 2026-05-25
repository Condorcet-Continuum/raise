// FICHIER : crates/raise-desktop/src/commands/gnn_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*;

// 🎯 L'état Tauri est conservé ici car le frontend y accède, mais la logique est dans raise-core
use raise_core::services::gnn_service;
pub use raise_core::services::gnn_service::GnnState;

use tauri::{command, State};

#[command]
pub async fn init_gnn_engine(
    state: State<'_, GnnState>,
    storage: State<'_, StorageEngine>,
    domain: String,
    db_name: String,
) -> RaiseResult<String> {
    // 🎯 FIX : Ajout de '&' pour passer domain et db_name par référence
    gnn_service::init_gnn_engine(state.inner(), storage.inner(), &domain, &db_name).await
}

#[command]
pub async fn train_gnn_step(
    state: State<'_, GnnState>,
    storage: State<'_, StorageEngine>,
    domain: String,
    db: String,
    lambda: f32,
) -> RaiseResult<f32> {
    // 🎯 FIX : Ajout de '&' pour passer domain et db par référence
    gnn_service::train_gnn_step(state.inner(), storage.inner(), &domain, &db, lambda).await
}

#[command]
pub async fn audit_ontology(
    state: State<'_, GnnState>,
    storage: State<'_, StorageEngine>,
    domain: String,
    db: String,
) -> RaiseResult<Vec<JsonValue>> {
    // 🎯 FIX : Ajout de '&' pour passer domain et db par référence
    gnn_service::audit_ontology(state.inner(), storage.inner(), &domain, &db).await
}
