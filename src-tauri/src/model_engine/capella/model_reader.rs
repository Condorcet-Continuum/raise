// FICHIER : src-tauri/src/model_engine/capella/model_reader.rs

use super::xmi_parser::CapellaXmiParser;
use crate::model_engine::types::{ProjectMeta, ProjectModel};
use anyhow::Result;
use chrono::Utc;
use std::path::Path;

pub struct CapellaReader;

impl CapellaReader {
    /// Lit un fichier .capella et retourne un ProjectModel complet
    pub fn read_model(path: &Path) -> Result<ProjectModel> {
        let mut model = ProjectModel::default();

        // 1. Parsing du XMI (Structure Sémantique)
        CapellaXmiParser::parse_file(path, &mut model)?;

        // 2. Remplissage des métadonnées
        let filename = path
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unknown.capella");

        model.meta = ProjectMeta {
            name: filename.to_string(),
            loaded_at: Utc::now().to_rfc3339(),
            element_count: Self::count_elements(&model),
        };

        Ok(model)
    }

    fn count_elements(model: &ProjectModel) -> usize {
        model.oa.actors.len()
            + model.oa.activities.len()
            + model.sa.components.len()
            + model.sa.functions.len()
            + model.la.components.len()
            + model.la.functions.len()
            + model.pa.components.len()
            + model.pa.functions.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_element_counting() {
        use crate::model_engine::types::{ArcadiaElement, NameType};
        use std::collections::HashMap;

        let mut model = ProjectModel::default();
        let dummy = ArcadiaElement {
            id: "1".into(),
            name: NameType::default(),
            kind: "test".into(),
            // CORRECTION : Ajout du champ manquant
            description: None,
            properties: HashMap::new(),
        };

        model.sa.components.push(dummy.clone());
        model.la.functions.push(dummy.clone());

        assert_eq!(CapellaReader::count_elements(&model), 2);
    }
}
