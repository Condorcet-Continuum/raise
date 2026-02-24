// FICHIER : src-tauri/src/model_engine/capella/model_reader.rs
use crate::utils::{io::Path, prelude::*};

use super::xmi_parser::CapellaXmiParser;
use crate::model_engine::types::{ProjectMeta, ProjectModel};

pub struct CapellaReader;

impl CapellaReader {
    /// Lit un fichier .capella et retourne un ProjectModel complet
    pub fn read_model(path: &Path) -> RaiseResult<ProjectModel> {
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
            // CORRECTION : Ajout des valeurs par défaut pour les champs manquants
            ..Default::default()
        };

        Ok(model)
    }

    fn count_elements(model: &ProjectModel) -> usize {
        // OA
        let oa = model.oa.actors.len()
            + model.oa.activities.len()
            + model.oa.capabilities.len()
            + model.oa.entities.len()
            + model.oa.exchanges.len();

        // SA
        let sa = model.sa.components.len()
            + model.sa.functions.len()
            + model.sa.actors.len()
            + model.sa.capabilities.len()
            + model.sa.exchanges.len();

        // LA
        let la = model.la.components.len()
            + model.la.functions.len()
            + model.la.actors.len()
            + model.la.interfaces.len()
            + model.la.exchanges.len();

        // PA
        let pa = model.pa.components.len()
            + model.pa.functions.len()
            + model.pa.actors.len()
            + model.pa.links.len()
            + model.pa.exchanges.len();

        // EPBS & DATA
        let others = model.epbs.configuration_items.len()
            + model.data.classes.len()
            + model.data.data_types.len()
            + model.data.exchange_items.len();

        // AJOUT : TRANSVERSE
        let transverse = model.transverse.requirements.len()
            + model.transverse.scenarios.len()
            + model.transverse.functional_chains.len()
            + model.transverse.constraints.len()
            + model.transverse.common_definitions.len()
            + model.transverse.others.len();

        oa + sa + la + pa + others + transverse
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use std::collections::HashMap;

    // Helper pour créer un élément dummy rapidement
    fn create_dummy(kind: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: "1".into(),
            name: NameType::default(),
            kind: kind.into(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_element_counting_with_transverse() {
        let mut model = ProjectModel::default();

        // Ajout d'éléments dans les couches classiques
        model.sa.components.push(create_dummy("SystemComponent"));
        model.la.functions.push(create_dummy("LogicalFunction"));

        // Ajout d'éléments dans la couche Transverse
        model
            .transverse
            .requirements
            .push(create_dummy("Requirement"));
        model.transverse.scenarios.push(create_dummy("Scenario"));
        model
            .transverse
            .functional_chains
            .push(create_dummy("FunctionalChain"));

        // Calcul attendu : 2 (SA+LA) + 3 (Transverse) = 5
        assert_eq!(CapellaReader::count_elements(&model), 5);
    }
}
