// FICHIER : crates/raise-desktop/src/commands/codegen_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*;

// 🎯 On importe le service métier
use raise_core::services::codegen_service;

// 🎯 L'état est maintenant importé directement depuis le noyau
use raise_core::services::rules_service::RuleEngineState;

use tauri::{command, State};

#[command]
pub async fn generate_source_code(
    element_id: String,
    domain: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<JsonValue> {
    // 🎯 FIX : Plus de lock().await ici ! On passe directement l'état (state.inner())
    codegen_service::generate_source_code(&element_id, &domain, state.inner(), storage.inner())
        .await
}

#[command]
pub async fn ingest_code_file(
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<usize> {
    // 🎯 FIX : On passe directement l'état
    codegen_service::ingest_code_file(&path, state.inner(), storage.inner()).await
}

#[command]
pub async fn weave_code_file(
    module_name: String,
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<String> {
    // 🎯 FIX : On passe directement l'état
    codegen_service::weave_code_file(&module_name, &path, state.inner(), storage.inner()).await
}
