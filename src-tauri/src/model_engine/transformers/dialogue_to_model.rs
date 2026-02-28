// FICHIER : src-tauri/src/model_engine/transformers/dialogue_to_model.rs

use crate::utils::{data::Value, prelude::*, HashMap, Uuid};

use crate::model_engine::arcadia; // <-- La source de vérité
use crate::model_engine::types::{ArcadiaElement, NameType};

/// Transformateur spécialisé pour convertir une intention IA (JSON) en Élément Arcadia
pub struct DialogueToModelTransformer;

impl DialogueToModelTransformer {
    /// Convertit un JSON d'intention (issu du LLM) en structure ArcadiaElement
    pub fn create_element_from_intent(intent: &Value) -> RaiseResult<ArcadiaElement> {
        // 1. Validation des champs requis
        // 1. Extraction du nom de l'intention
        let name_str = match intent.get("name") {
            Some(val) => match val.as_str() {
                Some(s) => s,
                None => raise_error!(
                    "ERR_INTENT_INVALID_FORMAT",
                    context = json!({
                        "field": "name",
                        "expected": "string",
                        "received": val,
                        "hint": "Le nom de l'intention doit être une chaîne de caractères."
                    })
                ),
            },
            None => raise_error!(
                "ERR_INTENT_MISSING_FIELD",
                context = json!({
                    "field": "name",
                    "hint": "Le champ 'name' est obligatoire pour identifier l'intention."
                })
            ),
        };

        // 2. Extraction du type de l'intention
        let type_str = match intent.get("type") {
            Some(val) => match val.as_str() {
                Some(s) => s,
                None => raise_error!(
                    "ERR_INTENT_INVALID_FORMAT",
                    context = json!({
                        "field": "type",
                        "expected": "string (ex: 'Component', 'Function')",
                        "received": val,
                        "hint": "Le type d'intention doit être une chaîne."
                    })
                ),
            },
            None => raise_error!(
                "ERR_INTENT_MISSING_FIELD",
                context = json!({
                    "field": "type",
                    "hint": "Le champ 'type' est requis pour déterminer la stratégie d'exécution."
                })
            ),
        };

        // 2. Déduction de la couche (Layer) par défaut si manquante
        let layer_str = intent
            .get("layer")
            .and_then(|v| v.as_str())
            .unwrap_or("Logical"); // Par défaut : Architecture Logique

        // 3. Résolution de l'URI Arcadia (Mapping Sémantique via constantes centralisées)
        let type_uri = match (layer_str, type_str) {
            // Logical Architecture
            ("Logical", "Component") => arcadia::KIND_LA_COMPONENT,
            ("Logical", "Function") => arcadia::KIND_LA_FUNCTION,
            ("Logical", "Actor") => arcadia::KIND_LA_ACTOR,

            // System Analysis
            ("System", "Function") => arcadia::KIND_SA_FUNCTION,
            ("System", "Component") => arcadia::KIND_SA_COMPONENT,
            ("System", "Actor") => arcadia::KIND_SA_ACTOR,

            // Physical Architecture
            ("Physical", "Component") => arcadia::KIND_PA_COMPONENT,
            ("Physical", "Function") => arcadia::KIND_PA_FUNCTION,
            ("Physical", "Link") => arcadia::KIND_PA_LINK,

            // Operational Analysis
            ("Operational", "Actor") => arcadia::KIND_OA_ACTOR,
            ("Operational", "Activity") => arcadia::KIND_OA_ACTIVITY,

            // Fallback ou erreur
            (l, t) => {
                raise_error!(
                    "ERR_SEMANTIC_LAYER_TYPE_MISMATCH",
                    error = format!("Combinaison Layer/Type non reconnue : {} / {}", l, t),
                    context = json!({
                        "layer": l,
                        "semantic_type": t,
                        "action": "resolve_factory_handler",
                        "hint": "Cette combinaison n'est pas enregistrée dans la fabrique sémantique. Vérifiez si le type est compatible avec la couche spécifiée."
                    })
                );
            }
        };

        // 4. Construction de l'objet Arcadia
        let properties = HashMap::new();

        let description = intent
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Création de l'élément
        Ok(ArcadiaElement {
            id: Uuid::new_v4().to_string(), // Génération auto de l'ID
            name: NameType::String(name_str.to_string()),
            kind: type_uri.to_string(),
            description,
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
        // Vérification avec la constante officielle
        assert_eq!(element.kind, arcadia::KIND_LA_COMPONENT);

        assert_eq!(element.description.as_deref(), Some("Gère le vol"));
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

        assert_eq!(element.kind, arcadia::KIND_LA_COMPONENT);
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
