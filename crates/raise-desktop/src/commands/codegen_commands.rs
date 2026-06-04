// FICHIER : crates/raise-desktop/src/commands/codegen_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*; // 🎯 Façade Unique RAISE

// 🎯 On importe le service métier
use raise_core::services::codegen_service;

// 🎯 L'état est importé directement depuis le noyau
use raise_core::services::rules_service::RuleEngineState;

use tauri::{command, State};

#[command]
pub async fn generate_source_code(
    element_id: String,
    domain: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>, // 🎯 FIX : Alignement sur le type géré par Tauri (Arc)
) -> RaiseResult<JsonValue> {
    // 🎯 Zéro Dette : On passe les références propres au service métier
    codegen_service::generate_source_code(
        &element_id,
        &domain,
        state.inner(),
        storage.inner().as_ref(), // 🎯 Déréférencement du SharedRef pour prêter le StorageEngine
    )
    .await
}

#[command]
pub async fn ingest_code_file(
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>, // 🎯 FIX
) -> RaiseResult<usize> {
    codegen_service::ingest_code_file(&path, state.inner(), storage.inner().as_ref(), false).await
}

#[command]
pub async fn weave_code_file(
    module_name: String,
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, SharedRef<StorageEngine>>, // 🎯 FIX
) -> RaiseResult<String> {
    codegen_service::weave_code_file(
        &module_name,
        &path,
        state.inner(),
        storage.inner().as_ref(),
        false,
    )
    .await
}
