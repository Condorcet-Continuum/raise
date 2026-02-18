// FICHIER : src-tauri/src/commands/traceability_commands.rs

use crate::utils::{data, prelude::*};

use crate::model_engine::types::ArcadiaElement;
use crate::AppState;
use tauri::State;

// Import des services du module de traçabilité
// Ces modules ont été mis à jour pour inclure la couche Transverse
use crate::traceability::{
    impact_analyzer::{ImpactAnalyzer, ImpactReport},
    reporting::{
        audit_report::{AuditGenerator, AuditReport},
        trace_matrix::{MatrixGenerator, TraceabilityMatrix},
    },
    tracer::Tracer,
};

/// Commande : Analyse d'Impact
/// Déclenche le calcul de propagation des changements à partir d'un élément racine.
/// Prend désormais en compte les liens vers les Exigences et Scénarios.
#[tauri::command]
pub fn analyze_impact(
    state: State<AppState>,
    element_id: String,
    depth: usize,
) -> Result<ImpactReport> {
    // Gestion robuste de l'erreur de Mutex (PoisonError)
    let model = state.model.lock().map_err(|e| e.to_string())?;

    // Initialisation du moteur de traçabilité
    let tracer = Tracer::new(&model);

    // Lancement de l'analyse
    let analyzer = ImpactAnalyzer::new(tracer);
    let report = analyzer.analyze(&element_id, depth);

    Ok(report)
}

/// Commande : Rapport d'Audit Global
/// Exécute tous les checkers de conformité (DO-178C, EU AI Act, etc.)
/// Retourne un rapport incluant les statistiques de la couche Transverse.
#[tauri::command]
pub fn run_compliance_audit(state: State<AppState>) -> Result<AuditReport> {
    let model = state.model.lock().map_err(|e| e.to_string())?;

    // AuditGenerator a été mis à jour pour compter les Requirements/Scenarios
    let report = AuditGenerator::generate(&model);

    Ok(report)
}

/// Commande : Matrice de Traçabilité
/// Génère la vue tabulaire de couverture SA -> LA
#[tauri::command]
pub fn get_traceability_matrix(state: State<AppState>) -> Result<TraceabilityMatrix> {
    let model = state.model.lock().map_err(|e| e.to_string())?;

    let matrix = MatrixGenerator::generate_sa_to_la(&model);

    Ok(matrix)
}

/// Commande : Navigation de Voisinage
/// Retourne les parents (Upstream) et les enfants (Downstream)
#[tauri::command]
pub fn get_element_neighbors(state: State<AppState>, element_id: String) -> Result<data::Value> {
    let model = state.model.lock().map_err(|e| e.to_string())?;

    let tracer = Tracer::new(&model);

    // Récupération des références (incluant potentiellement des liens transverses)
    let upstream_refs = tracer.get_upstream_elements(&element_id);
    let downstream_refs = tracer.get_downstream_elements(&element_id);

    // Clonage pour le DTO (ArcadiaElement implémente Clone)
    let upstream: Vec<ArcadiaElement> = upstream_refs.into_iter().cloned().collect();
    let downstream: Vec<ArcadiaElement> = downstream_refs.into_iter().cloned().collect();

    Ok(serde_json::json!({
        "center_id": element_id,
        "upstream": upstream,
        "downstream": downstream
    }))
}
