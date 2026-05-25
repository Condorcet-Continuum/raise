// FICHIER : crates/raise-desktop/src/commands/traceability_commands.rs

use raise_core::traceability::impact_analyzer::ImpactReport;
use raise_core::traceability::reporting::{
    audit_report::AuditReport, trace_matrix::TraceabilityMatrix,
};
use raise_core::utils::prelude::*;

// 🎯 On importe le service depuis le noyau
use raise_core::services::traceability_service;

// 🎯 On importe l'état applicatif local du Desktop
use crate::AppState;

use tauri::{command, State};

#[command]
pub async fn analyze_impact(
    state: State<'_, SharedRef<AppState>>,
    element_id: String,
    depth: usize,
) -> RaiseResult<ImpactReport> {
    let model = state.model.lock().await;
    traceability_service::analyze_impact(&model, &element_id, depth).await
}

#[command]
pub async fn run_compliance_audit(
    state: State<'_, SharedRef<AppState>>,
) -> RaiseResult<AuditReport> {
    let model = state.model.lock().await;
    traceability_service::run_compliance_audit(&model).await
}

#[command]
pub async fn get_traceability_matrix(
    state: State<'_, SharedRef<AppState>>,
) -> RaiseResult<TraceabilityMatrix> {
    let model = state.model.lock().await;
    traceability_service::get_traceability_matrix(&model).await
}

#[command]
pub async fn get_element_neighbors(
    state: State<'_, SharedRef<AppState>>,
    element_id: String,
) -> RaiseResult<JsonValue> {
    let model = state.model.lock().await;
    traceability_service::get_element_neighbors(&model, &element_id).await
}
