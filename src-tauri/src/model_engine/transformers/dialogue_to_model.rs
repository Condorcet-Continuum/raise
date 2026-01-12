// FICHIER : src-tauri/src/model_engine/transformers/dialogue_to_model.rs

use crate::model_engine::types::{ArcadiaElement, NameType};
use anyhow::{anyhow, Result};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

// URIs Arcadia (Simplifiés pour cet exemple, à synchroniser avec vocabulaire.rs)
const URI_LOGICAL_COMPONENT: &str = "https://raise.io/ontology/arcadia/la#LogicalComponent";
const URI_SYSTEM_FUNCTION: &str = "https://raise.io/ontology/arcadia/sa#SystemFunction";
const URI_LOGICAL_FUNCTION: &str = "https://raise.io/ontology/arcadia/la#LogicalFunction";
const URI_PHYSICAL_COMPONENT: &str = "https://raise.io/ontology/arcadia/pa#PhysicalComponent";

/// Transformateur spécialisé pour convertir une intention IA (JSON) en Élément Arcadia
pub struct DialogueToModelTransformer;

impl DialogueToModelTransformer {
    /// Convertit un JSON d'intention (issu du LLM) en structure ArcadiaElement
    pub fn create_element_from_intent(intent: &Value) -> Result<ArcadiaElement> {
        // 1. Validation des champs requis
        let name_str = intent
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Le champ 'name' est requis dans l'intention."))?;

        let type_str = intent
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow!("Le champ 'type' est requis (ex: Component, Function)."))?;

        // 2. Déduction de la couche (Layer) par défaut si manquante
        let layer_str = intent
            .get("layer")
            .and_then(|v| v.as_str())
            .unwrap_or("Logical"); // Par défaut : Architecture Logique

        // 3. Résolution de l'URI Arcadia (Mapping Sémantique)
        let type_uri = match (layer_str, type_str) {
            ("Logical", "Component") => URI_LOGICAL_COMPONENT,
            ("Physical", "Component") => URI_PHYSICAL_COMPONENT,
            ("System", "Function") => URI_SYSTEM_FUNCTION,
            ("Logical", "Function") => URI_LOGICAL_FUNCTION,
            // Fallback ou erreur
            (l, t) => {
                return Err(anyhow!(
                    "Combinaison Layer/Type non supportée : {} / {}",
                    l,
                    t
                ))
            }
        };

        // 4. Construction de l'objet Arcadia
        let properties = HashMap::new();

        // CORRECTION : Extraction de la description vers le champ dédié
        let description = intent
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Création de l'élément
        Ok(ArcadiaElement {
            id: Uuid::new_v4().to_string(), // Génération auto de l'ID
            name: NameType::String(name_str.to_string()),
            kind: type_uri.to_string(),
            description, // Champ ajouté
            properties,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_create_logical_component_from_intent() {
        // Intention simulée venant de l'IA
        let intent = json!({
            "type": "Component",
            "layer": "Logical",
            "name": "FlightManager",
            "description": "Gère le vol"
        });

        let element = DialogueToModelTransformer::create_element_from_intent(&intent)
            .expect("Should create element");

        assert_eq!(element.name.as_str(), "FlightManager");
        assert_eq!(element.kind, URI_LOGICAL_COMPONENT);

        // Vérif Description (nouveau champ)
        assert_eq!(element.description.as_deref(), Some("Gère le vol"));

        // Vérif ID généré
        assert!(!element.id.is_empty());
    }

    #[test]
    fn test_default_layer_fallback() {
        // Intention incomplète (pas de layer) -> Doit défaut à Logical
        let intent = json!({
            "type": "Component", // Pas de "layer"
            "name": "SimpleBox"
        });

        let element = DialogueToModelTransformer::create_element_from_intent(&intent)
            .expect("Should create element with default layer");

        assert_eq!(element.kind, URI_LOGICAL_COMPONENT);
    }

    #[test]
    fn test_error_on_unknown_type() {
        let intent = json!({
            "type": "Unicorn", // Type inconnu
            "name": "BadElement"
        });

        let result = DialogueToModelTransformer::create_element_from_intent(&intent);
        assert!(result.is_err());
    }
}
