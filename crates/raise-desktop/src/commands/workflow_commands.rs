// FICHIER : crates/raise-desktop/src/commands/workflow_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::utils::prelude::*;
use raise_core::workflow_engine::WorkflowDefinition;

// 🎯 On importe le service et les DTOs depuis le noyau
use raise_core::services::workflow_service::{self, WorkflowStore, WorkflowView};

use tauri::{command, State};

#[command]
pub async fn set_sensor_value(
    storage: State<'_, SharedRef<StorageEngine>>,
    value: f64,
) -> RaiseResult<String> {
    workflow_service::set_sensor_value(storage.inner(), value).await
}

#[command]
pub async fn compile_mission(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mission_id: String,
) -> RaiseResult<String> {
    workflow_service::compile_mission(storage.inner(), state.inner(), &mission_id).await
}

#[command]
pub async fn register_workflow(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    definition: WorkflowDefinition,
) -> RaiseResult<String> {
    workflow_service::register_workflow(state.inner(), definition).await
}

#[command]
pub async fn start_workflow(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    mission_id: String,
    workflow_handle: String,
) -> RaiseResult<WorkflowView> {
    workflow_service::start_workflow(
        storage.inner(),
        state.inner(),
        mission_id.to_string(),
        workflow_handle.to_string(),
    )
    .await
}

#[command]
pub async fn resume_workflow(
    storage: State<'_, SharedRef<StorageEngine>>,
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String,
    node_id: String,
    approved: bool,
) -> RaiseResult<WorkflowView> {
    workflow_service::resume_workflow(
        storage.inner(),
        state.inner(),
        &instance_handle,
        &node_id,
        approved,
    )
    .await
}

#[command]
pub async fn get_workflow_state(
    state: State<'_, AsyncMutex<WorkflowStore>>,
    instance_handle: String,
) -> RaiseResult<WorkflowView> {
    workflow_service::get_workflow_state(state.inner(), &instance_handle).await
}
