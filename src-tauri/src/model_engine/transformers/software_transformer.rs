// FICHIER : src-tauri/src/model_engine/transformers/software_transformer.rs

use super::ModelTransformer;
use crate::model_engine::arcadia; // <-- Accès au vocabulaire sémantique
use anyhow::Result;
use serde_json::{json, Value};

pub struct SoftwareTransformer;

impl ModelTransformer for SoftwareTransformer {
    fn transform(&self, element: &Value) -> Result<Value> {
        // 1. Extraction des métadonnées de base via constantes
        let name = element
            .get(arcadia::PROP_NAME)
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownElement");

        let id = element
            .get(arcadia::PROP_ID)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 2. Transformation des Fonctions allouées en Méthodes
        let mut methods = Vec::new();
        // Note: La clé "ownedFunctionalAllocation" est spécifique au métamodèle technique,
        // distincte de "allocatedFunctions" (résultat logique). On garde la clé brute ici.
        if let Some(allocations) = element
            .get("ownedFunctionalAllocation")
            .and_then(|v| v.as_array())
        {
            for func in allocations {
                let fname = func.get(arcadia::PROP_NAME).and_then(|v| v.as_str());
                let fid = func.get(arcadia::PROP_ID).and_then(|v| v.as_str());

                if let (Some(n), Some(i)) = (fname, fid) {
                    methods.push(json!({
                        "name": n,
                        "id": i,
                        "visibility": "pub",
                        "return_type": "Result<()>",
                        "description": format!("Implémentation de la fonction {}", n)
                    }));
                }
            }
        }

        // 3. Transformation des Sous-composants en Champs (Composition)
        let mut fields = Vec::new();

        // Utilisation des constantes pour cibler les bonnes collections
        let logical_children = element
            .get(arcadia::PROP_OWNED_LOGICAL_COMPONENTS)
            .and_then(|v| v.as_array());

        let system_children = element
            .get(arcadia::PROP_OWNED_SYSTEM_COMPONENTS)
            .and_then(|v| v.as_array());

        let all_children = logical_children
            .into_iter()
            .flatten()
            .chain(system_children.into_iter().flatten());

        for child in all_children {
            let cname = child.get(arcadia::PROP_NAME).and_then(|v| v.as_str());
            let cid = child.get(arcadia::PROP_ID).and_then(|v| v.as_str());

            if let (Some(n), Some(i)) = (cname, cid) {
                fields.push(json!({
                    "name": n.to_lowercase(),
                    "type": n,
                    "id": i,
                    "visibility": "private"
                }));
            }
        }

        // 4. Gestion de l'héritage
        let parent_class = element
            .get("base_class") // Pas encore de constante
            .and_then(|obj| obj.get(arcadia::PROP_NAME))
            .and_then(|v| v.as_str());

        // 5. Construction de l'objet final
        Ok(json!({
            "domain": "software",
            "meta": {
                "uuid": id,
                "source_element": name,
                "generated_at": chrono::Utc::now().to_rfc3339()
            },
            "entity": {
                "name": name,
                "kind": "class",
                "methods": methods,
                "fields": fields,
                "parent": parent_class
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia; // Important pour les tests

    #[test]
    fn test_software_transformation_methods_and_fields() {
        let transformer = SoftwareTransformer;

        // Mock utilisant les constantes pour garantir la synchro
        let component = json!({
            arcadia::PROP_ID: "UUID_COMP_1",
            arcadia::PROP_NAME: "FlightController",
            // Clés spécifiques
            "ownedFunctionalAllocation": [
                { arcadia::PROP_ID: "UUID_F1", arcadia::PROP_NAME: "ComputeAltitude" },
                { arcadia::PROP_ID: "UUID_F2", arcadia::PROP_NAME: "LogData" }
            ],
            arcadia::PROP_OWNED_LOGICAL_COMPONENTS: [
                { arcadia::PROP_ID: "UUID_SUB1", arcadia::PROP_NAME: "GPSModule" }
            ],
            "base_class": { arcadia::PROP_ID: "UUID_PARENT", arcadia::PROP_NAME: "GenericController" }
        });

        let result = transformer
            .transform(&component)
            .expect("Transformation failed");

        assert_eq!(result["domain"], "software");
        assert_eq!(result["entity"]["name"], "FlightController");
        assert_eq!(result["entity"]["parent"], "GenericController");

        let methods = result["entity"]["methods"].as_array().unwrap();
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0]["name"], "ComputeAltitude");

        let fields = result["entity"]["fields"].as_array().unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0]["type"], "GPSModule");
    }

    #[test]
    fn test_software_transformation_minimal_element() {
        let transformer = SoftwareTransformer;
        let component = json!({
            arcadia::PROP_ID: "UUID_EMPTY",
            arcadia::PROP_NAME: "EmptyBox"
        });

        let result = transformer
            .transform(&component)
            .expect("Should handle minimal input");

        assert_eq!(result["entity"]["name"], "EmptyBox");
    }
}
