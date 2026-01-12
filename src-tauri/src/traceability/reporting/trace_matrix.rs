// FICHIER : src-tauri/src/traceability/reporting/trace_matrix.rs

use crate::model_engine::types::{NameType, ProjectModel};
use crate::traceability::tracer::Tracer;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct TraceabilityMatrix {
    pub rows: Vec<TraceRow>,
}

#[derive(Debug, Serialize)]
pub struct TraceRow {
    pub source_id: String,
    pub source_name: String,
    pub target_ids: Vec<String>,
    pub coverage_status: String, // "Covered", "Partial", "Uncovered"
}

pub struct MatrixGenerator;

impl MatrixGenerator {
    /// Génère une matrice SA -> LA (Fonctions Système vers Composants Logiques)
    pub fn generate_sa_to_la(model: &ProjectModel) -> TraceabilityMatrix {
        let tracer = Tracer::new(model);
        let mut rows = Vec::new();

        for func in &model.sa.functions {
            // 1. Identification des éléments aval (Downstream)
            let realized_by = tracer.get_downstream_elements(&func.id);

            // 2. Extraction des noms (avec gestion NameType)
            let targets: Vec<String> = realized_by
                .iter()
                .map(|e| match &e.name {
                    NameType::String(s) => s.clone(),
                    _ => "Nom Inconnu".to_string(),
                })
                .collect();

            // 3. Calcul du statut
            let status = if targets.is_empty() {
                "Uncovered".to_string()
            } else {
                "Covered".to_string()
            };

            // 4. Extraction du nom source
            let source_name = match &func.name {
                NameType::String(s) => s.clone(),
                _ => "Nom Inconnu".to_string(),
            };

            rows.push(TraceRow {
                source_id: func.id.clone(),
                source_name,
                target_ids: targets,
                coverage_status: status,
            });
        }

        TraceabilityMatrix { rows }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel};
    use serde_json::json;
    use std::collections::HashMap;

    // Helper pour créer des éléments mockés
    fn create_element(id: &str, name: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            name: NameType::String(name.to_string()),
            kind: "Element".to_string(),
            // CORRECTION : Initialisation du champ description ajouté récemment
            description: None,
            properties,
        }
    }

    #[test]
    fn test_matrix_generation_sa_to_la() {
        let mut model = ProjectModel::default();

        // Cas 1 : Fonction couverte (Lien 'allocatedTo')
        let func_covered = create_element("f1", "Fonction 1", json!({ "allocatedTo": ["comp1"] }));

        // Cas 2 : Fonction orpheline
        let func_orphan = create_element("f2", "Fonction 2", json!({}));

        // Cible (doit exister pour le Tracer)
        let comp_target = create_element("comp1", "Composant 1", json!({}));

        model.sa.functions = vec![func_covered, func_orphan];
        model.la.components = vec![comp_target];

        let matrix = MatrixGenerator::generate_sa_to_la(&model);

        assert_eq!(matrix.rows.len(), 2);

        // Vérification Ligne 1
        let row1 = matrix.rows.iter().find(|r| r.source_id == "f1").unwrap();
        assert_eq!(row1.coverage_status, "Covered");
        assert!(row1.target_ids.contains(&"Composant 1".to_string()));

        // Vérification Ligne 2
        let row2 = matrix.rows.iter().find(|r| r.source_id == "f2").unwrap();
        assert_eq!(row2.coverage_status, "Uncovered");
        assert!(row2.target_ids.is_empty());
    }
}
