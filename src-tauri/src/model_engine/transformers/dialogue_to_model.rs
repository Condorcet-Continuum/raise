// FICHIER : src-tauri/src/model_engine/transformers/dialogue_to_model.rs

use crate::model_engine::arcadia::ArcadiaOntology;
use crate::model_engine::types::{ArcadiaElement, NameType};
use crate::utils::prelude::*;

/// Transformateur spécialisé pour convertir une intention IA (JSON) en Élément Arcadia
pub struct DialogueToModelTransformer;

impl DialogueToModelTransformer {
    /// Convertit un JSON d'intentions (issu du LLM) en structure ArcadiaElement
    pub fn create_element_from_intent(intent: &JsonValue) -> RaiseResult<ArcadiaElement> {
        // 1. Extraction du nom
        let Some(name_str) = intent.get("name").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_INTENT_MISSING_NAME",
                context =
                    json_value!({ "intent": intent, "hint": "Le champ 'name' est obligatoire." })
            );
        };

        // 2. Extraction du type de base
        let Some(type_str) = intent.get("type").and_then(|v| v.as_str()) else {
            raise_error!(
                "ERR_INTENT_MISSING_TYPE",
                context =
                    json_value!({ "intent": intent, "hint": "Le champ 'type' est obligatoire." })
            );
        };

        // 3. Déduction de la couche (Layer)
        let layer_str = intent
            .get("layer")
            .and_then(|v| v.as_str())
            .unwrap_or("Logical");

        // 4. Mapping des termes simplifiés vers les classes de l'ontologie
        let (prefix, class_name) = match (layer_str, type_str) {
            ("Logical", "Component") => ("la", "LogicalComponent"),
            ("Logical", "Function") => ("la", "LogicalFunction"),
            ("Logical", "Actor") => ("la", "LogicalActor"),

            ("System", "Function") => ("sa", "SystemFunction"),
            ("System", "Component") => ("sa", "SystemComponent"),
            ("System", "Actor") => ("sa", "SystemActor"),

            ("Physical", "Component") => ("pa", "PhysicalComponent"),
            ("Physical", "Function") => ("pa", "PhysicalFunction"),
            ("Physical", "Link") => ("pa", "PhysicalLink"),

            ("Operational", "Actor") => ("oa", "OperationalActor"),
            ("Operational", "Activity") => ("oa", "OperationalActivity"),

            (l, t) => {
                raise_error!(
                    "ERR_SEMANTIC_MAPPING_NOT_FOUND",
                    error = format!("Combinaison Layer/Type inconnue : {}/{}", l, t)
                );
            }
        };

        // 🎯 FIX : Utilisation de let-else pour la résolution dynamique
        let Some(type_uri) = ArcadiaOntology::get_uri(prefix, class_name) else {
            raise_error!(
                "ERR_SEMANTIC_URI_UNRESOLVED",
                error = format!(
                    "Impossible de résoudre l'URI pour {}:{}",
                    prefix, class_name
                ),
                context = json_value!({ "prefix": prefix, "class": class_name })
            );
        };

        // 5. Construction de l'objet Arcadia
        let description = intent
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Ok(ArcadiaElement {
            id: UniqueId::new_v4().to_string(),
            name: NameType::String(name_str.to_string()),
            kind: type_uri,
            description,
            properties: UnorderedMap::new(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::ArcadiaOntology;

    #[test]
    fn test_create_logical_component_from_intent() {
        let intent = json_value!({
            "type": "Component",
            "layer": "Logical",
            "name": "FlightManager"
        });

        let element = DialogueToModelTransformer::create_element_from_intent(&intent)
            .expect("Should create element");

        assert_eq!(element.name.as_str(), "FlightManager");

        let expected_uri = ArcadiaOntology::get_uri("la", "LogicalComponent").unwrap();
        assert_eq!(element.kind, expected_uri);
    }
}
