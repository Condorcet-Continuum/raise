// FICHIER : crates/raise-desktop/src/commands/codegen_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*; // 🎯 Façade Unique RAISE

use raise_core::services::codegen_service;
use raise_core::services::rules_service::RuleEngineState;
use tauri::{command, State};

// 🎯 Helper interne pour déballer le contexte depuis le RuleEngine
async fn get_active_context(state: &State<'_, RuleEngineState>) -> (String, String) {
    let model_guard = state.inner().model.lock().await;
    codegen_service::resolve_active_context(&model_guard)
}

#[command]
pub async fn generate_source_code(
    element_id: String,
    target_domain: String, // 🎯 L'UI envoie "software" ou "hardware"
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>,
) -> RaiseResult<JsonValue> {
    let (domain, db) = get_active_context(&state).await;

    codegen_service::generate_source_code(
        &element_id,
        &target_domain,
        &domain,
        &db,
        storage.inner().as_ref(),
    )
    .await
}

#[command]
pub async fn ingest_module(
    module_handle: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>,
) -> RaiseResult<usize> {
    let (domain, db) = get_active_context(&state).await;
    codegen_service::ingest_module(
        &module_handle,
        &domain,
        &db,
        storage.inner().as_ref(),
        false,
    )
    .await
}

#[command]
pub async fn weave_module(
    module_handle: String, // 🎯 Disparition de module_name et path
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>,
) -> RaiseResult<String> {
    let (domain, db) = get_active_context(&state).await;
    
    // 🎯 Appel de la nouvelle méthode du service
    codegen_service::weave_module(
        &module_handle,
        &domain,
        &db,
        storage.inner().as_ref(),
        false,
    )
    .await
}

#[command]
pub async fn auto_tag_module(
    module_handle: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>,
) -> RaiseResult<usize> {
    let (domain, db) = get_active_context(&state).await;
    codegen_service::auto_tag_module(&module_handle, &domain, &db, storage.inner().as_ref()).await
}
