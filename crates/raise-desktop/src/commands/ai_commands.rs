// FICHIER : crates/raise-desktop/src/commands/ai_commands.rs

use raise_core::ai::agents::AgentResult;
use raise_core::ai::llm::NativeLlmState;
use raise_core::ai::training::dataset::TrainingExample;
use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*;

// 🎯 On importe les services et états depuis le noyau
use raise_core::services::ai_service::{self, AiState};

use tauri::{command, State};

/// 🖥️ COMMANDE TAURI : Expose la logique blueprint à l'interface graphique.
#[command]
pub async fn ai_execute_blueprint(
    storage: State<'_, SharedRef<StorageEngine>>,
    ai_state: State<'_, AiState>,
    domain: String,
    db: String,
    prompt_handle: String,
    vars: Option<JsonValue>,
) -> RaiseResult<String> {
    let storage_ref = storage.inner().clone();
    // 🎯 On délègue tout au noyau en passant des références
    ai_service::ai_execute_blueprint(
        storage_ref,
        ai_state.inner(),
        &domain,
        &db,
        &prompt_handle,
        vars,
    )
    .await
}

/// 📤 COMMANDE TAURI : Exporte un dataset d'entraînement pour un domaine spécifique.
#[command]
pub async fn ai_export_dataset(
    storage: State<'_, SharedRef<StorageEngine>>,
    space: String,
    db_name: String,
    domain: String,
) -> RaiseResult<Vec<TrainingExample>> {
    let storage_ref = storage.inner().clone();
    ai_service::ai_export_dataset(storage_ref.as_ref(), &space, &db_name, &domain).await
}

// --- COMMANDES ORCHESTRATION UNIFIÉE (V2) ---

#[command]
pub async fn ai_reset(ai_state: State<'_, AiState>) -> RaiseResult<()> {
    ai_service::ai_reset(ai_state.inner()).await
}

#[command]
pub async fn ai_learn_text(
    ai_state: State<'_, AiState>,
    content: String,
    source: String,
) -> RaiseResult<String> {
    ai_service::ai_learn_text(ai_state.inner(), &content, &source).await
}

#[command]
pub async fn ai_confirm_learning(
    ai_state: State<'_, AiState>,
    action_intent: String,
    entity_name: String,
    entity_kind: String,
) -> RaiseResult<String> {
    ai_service::ai_confirm_learning(ai_state.inner(), &action_intent, entity_name, entity_kind)
        .await
}

#[command]
pub async fn ai_chat(ai_state: State<'_, AiState>, user_input: String) -> RaiseResult<AgentResult> {
    ai_service::ai_chat(ai_state.inner(), &user_input).await
}

#[command]
pub async fn ask_native_llm(
    state: State<'_, NativeLlmState>,
    sys: String,
    usr: String,
) -> RaiseResult<String> {
    ai_service::ask_native_llm(state.inner(), &sys, &usr).await
}

#[command]
pub async fn validate_arcadia_gnn(
    collections_path: String,
    uri_a: String,
    uri_b: String,
) -> RaiseResult<JsonValue> {
    ai_service::validate_arcadia_gnn(&collections_path, &uri_a, &uri_b).await
}
