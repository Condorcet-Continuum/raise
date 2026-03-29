// FICHIER : src-tauri/src/commands/traceability_commands.rs

use crate::model_engine::types::ProjectModel;
use crate::utils::prelude::*;
use crate::AppState;

use crate::traceability::{
    impact_analyzer::{ImpactAnalyzer, ImpactReport},
    reporting::{
        audit_report::{AuditGenerator, AuditReport},
        trace_matrix::{MatrixGenerator, TraceabilityMatrix},
    },
    tracer::Tracer,
};

/// Helper interne : Convertit le modèle Arcadia en index de documents JSON
/// 🎯 PURE GRAPH : On utilise l'itérateur universel pour collecter tous les éléments
fn get_model_docs(model: &ProjectModel) -> UnorderedMap<String, JsonValue> {
    let mut docs = UnorderedMap::new();

    for e in model.all_elements() {
        if let Ok(val) = json::serialize_to_value(e) {
            docs.insert(e.id.clone(), val);
        }
    }

    docs
}

#[tauri::command]
pub async fn analyze_impact(
    state: tauri::State<'_, AppState>,
    element_id: String,
    depth: usize,
) -> RaiseResult<ImpactReport> {
    let model = state.model.lock().await;

    // Utilisation du constructeur de rétro-compatibilité (qui utilise all_elements désormais)
    let tracer = Tracer::from_legacy_model(&model);

    let analyzer = ImpactAnalyzer::new(tracer);
    let report = analyzer.analyze(&element_id, depth);

    Ok(report)
}

#[tauri::command]
pub async fn run_compliance_audit(state: tauri::State<'_, AppState>) -> RaiseResult<AuditReport> {
    let model = state.model.lock().await;

    // Préparation des données pour le générateur universel
    let docs = get_model_docs(&model);
    let tracer = Tracer::from_json_list(docs.values().cloned().collect());

    // AuditGenerator prend désormais 3 arguments
    let report = AuditGenerator::generate(&tracer, &docs, &model.meta.name);

    Ok(report)
}

#[tauri::command]
pub async fn get_traceability_matrix(
    state: tauri::State<'_, AppState>,
) -> RaiseResult<TraceabilityMatrix> {
    let model = state.model.lock().await;

    let docs = get_model_docs(&model);
    let tracer = Tracer::from_json_list(docs.values().cloned().collect());

    // Utilisation du générateur de couverture universel
    let matrix = MatrixGenerator::generate_coverage(&tracer, &docs, "Function");

    Ok(matrix)
}

#[tauri::command]
pub async fn get_element_neighbors(
    state: tauri::State<'_, AppState>,
    element_id: String,
) -> RaiseResult<JsonValue> {
    let model = state.model.lock().await;
    let docs = get_model_docs(&model);

    // Utilisation du nouveau Tracer
    let tracer = Tracer::from_legacy_model(&model);

    // Récupération des IDs
    let upstream_ids = tracer.get_upstream_ids(&element_id);
    let downstream_ids = tracer.get_downstream_ids(&element_id);

    // Résolution des objets complets via l'index
    let mut upstream = Vec::new();
    for id in upstream_ids {
        if let Some(val) = docs.get(&id) {
            upstream.push(val.clone());
        }
    }

    let mut downstream = Vec::new();
    for id in downstream_ids {
        if let Some(val) = docs.get(&id) {
            downstream.push(val.clone());
        }
    }

    Ok(json_value!({
        "center_id": element_id,
        "upstream": upstream,
        "downstream": downstream
    }))
}
