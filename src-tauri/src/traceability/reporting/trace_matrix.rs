use crate::model_engine::types::ProjectModel;
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
    /// Génère une matrice montrant comment les Fonctions Système (SA) sont couvertes par les Composants Logiques (LA).
    pub fn generate_sa_to_la(model: &ProjectModel) -> TraceabilityMatrix {
        let tracer = Tracer::new(model);
        let mut rows = Vec::new();

        for func in &model.sa.functions {
            // On cherche les éléments "downstream" (qui réalisent cette fonction)
            let realized_by = tracer.get_downstream_elements(&func.id);

            // [CORRECTION] Utilisation de .name.as_str() pour gérer le NameType
            let targets: Vec<String> = realized_by
                .iter()
                .map(|e| e.name.as_str().to_string())
                .collect();

            let status = if targets.is_empty() {
                "Uncovered".to_string()
            } else {
                "Covered".to_string()
            };

            rows.push(TraceRow {
                source_id: func.id.clone(),
                // [CORRECTION] Conversion NameType -> String
                source_name: func.name.as_str().to_string(),
                target_ids: targets,
                coverage_status: status,
            });
        }

        TraceabilityMatrix { rows }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType, ProjectModel}; // [CORRECTION] Import NameType
    use serde_json::json;
    use std::collections::HashMap;

    // Helper pour créer un élément rapidement
    // [CORRECTION] Pas de Default, construction manuelle
    fn create_element(id: &str, name: &str, props: serde_json::Value) -> ArcadiaElement {
        let mut properties = HashMap::new();
        if let Some(obj) = props.as_object() {
            for (k, v) in obj {
                properties.insert(k.clone(), v.clone());
            }
        }
        ArcadiaElement {
            id: id.to_string(),
            // [CORRECTION] Envelopper dans l'enum
            name: NameType::String(name.to_string()),
            // [CORRECTION] Champ kind obligatoire
            kind: "Element".to_string(),
            properties,
        }
    }

    #[test]
    fn test_matrix_sa_to_la_coverage() {
        let mut model = ProjectModel::default();

        // 1. Fonction SA couverte (elle pointe vers un composant LA)
        let sa_func_covered = create_element(
            "sa_func_1",
            "Syst Function 1",
            json!({ "allocatedTo": ["la_comp_1"] }),
        );

        // 2. Fonction SA non couverte (aucun lien)
        let sa_func_uncovered = create_element("sa_func_2", "Syst Function 2", json!({}));

        // 3. Le composant LA cible (doit exister pour que le Tracer le trouve)
        let la_comp = create_element("la_comp_1", "Logical Comp 1", json!({}));

        model.sa.functions = vec![sa_func_covered, sa_func_uncovered];
        model.la.components = vec![la_comp];

        // Génération de la matrice
        let matrix = MatrixGenerator::generate_sa_to_la(&model);

        assert_eq!(matrix.rows.len(), 2, "La matrice doit contenir 2 lignes");

        // Vérification Ligne 1 : Covered
        let row_1 = matrix
            .rows
            .iter()
            .find(|r| r.source_id == "sa_func_1")
            .unwrap();
        assert_eq!(row_1.coverage_status, "Covered");
        assert!(row_1.target_ids.contains(&"Logical Comp 1".to_string()));

        // Vérification Ligne 2 : Uncovered
        let row_2 = matrix
            .rows
            .iter()
            .find(|r| r.source_id == "sa_func_2")
            .unwrap();
        assert_eq!(row_2.coverage_status, "Uncovered");
        assert!(row_2.target_ids.is_empty());
    }
}
