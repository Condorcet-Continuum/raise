// FICHIER : src-tauri/src/commands/traceability_commands.rs

use crate::model_engine::types::{ArcadiaElement, ProjectModel};
use crate::utils::{data, prelude::*, HashMap};
use crate::AppState;
use tauri::State;

use crate::traceability::{
    impact_analyzer::{ImpactAnalyzer, ImpactReport},
    reporting::{
        audit_report::{AuditGenerator, AuditReport},
        trace_matrix::{MatrixGenerator, TraceabilityMatrix},
    },
    tracer::Tracer,
};

/// Helper interne : Convertit le modèle Arcadia en index de documents JSON
/// 🎯 Indispensable pour alimenter les nouveaux générateurs découplés.
fn get_model_docs(model: &ProjectModel) -> HashMap<String, Value> {
    let mut docs = HashMap::new();
    let mut collect = |elements: &Vec<ArcadiaElement>| {
        for e in elements {
            if let Ok(val) = data::to_value(e) {
                docs.insert(e.id.clone(), val);
            }
        }
    };

    // Collecte sur toutes les couches
    collect(&model.sa.functions);
    collect(&model.sa.components);
    collect(&model.la.functions);
    collect(&model.la.components);
    collect(&model.pa.functions);
    collect(&model.pa.components);
    // Couche Transverse
    collect(&model.transverse.requirements);
    collect(&model.transverse.scenarios);

    docs
}

#[tauri::command]
pub fn analyze_impact(
    state: State<AppState>,
    element_id: String,
    depth: usize,
) -> Result<ImpactReport> {
    let model = state.model.lock().map_err(|e| e.to_string())?;

    // 🎯 FIX : Utilisation du constructeur de rétro-compatibilité
    let tracer = Tracer::from_legacy_model(&model);

    let analyzer = ImpactAnalyzer::new(tracer);
    let report = analyzer.analyze(&element_id, depth);

    Ok(report)
}

#[tauri::command]
pub fn run_compliance_audit(state: State<AppState>) -> Result<AuditReport> {
    let model = state.model.lock().map_err(|e| e.to_string())?;

    // 🎯 FIX : Préparation des données pour le générateur universel
    let docs = get_model_docs(&model);
    let tracer = Tracer::from_json_list(docs.values().cloned().collect());

    // AuditGenerator prend désormais 3 arguments
    let report = AuditGenerator::generate(&tracer, &docs, &model.meta.name);

    Ok(report)
}

#[tauri::command]
pub fn get_traceability_matrix(state: State<AppState>) -> Result<TraceabilityMatrix> {
    let model = state.model.lock().map_err(|e| e.to_string())?;

    let docs = get_model_docs(&model);
    let tracer = Tracer::from_json_list(docs.values().cloned().collect());

    // 🎯 FIX : Utilisation du générateur de couverture universel
    let matrix = MatrixGenerator::generate_coverage(&tracer, &docs, "Function");

    Ok(matrix)
}

#[tauri::command]
pub fn get_element_neighbors(state: State<AppState>, element_id: String) -> Result<data::Value> {
    let model = state.model.lock().map_err(|e| e.to_string())?;
    let docs = get_model_docs(&model);

    // 🎯 FIX : Utilisation du nouveau Tracer
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

    Ok(serde_json::json!({
        "center_id": element_id,
        "upstream": upstream,
        "downstream": downstream
    }))
}
