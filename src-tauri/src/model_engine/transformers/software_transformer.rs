use super::ModelTransformer;
use anyhow::Result;
use serde_json::{json, Value};

pub struct SoftwareTransformer;

impl ModelTransformer for SoftwareTransformer {
    fn transform(&self, element: &Value) -> Result<Value> {
        // 1. Extraction des métadonnées de base
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("UnknownElement");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("");

        // 2. Transformation des Fonctions allouées en Méthodes
        let mut methods = Vec::new();
        if let Some(allocations) = element
            .get("ownedFunctionalAllocation")
            .and_then(|v| v.as_array())
        {
            for func in allocations {
                // On suppose que l'élément est hydraté (objet complet) ou contient au moins le nom
                let fname = func.get("name").and_then(|v| v.as_str());
                let fid = func.get("id").and_then(|v| v.as_str());

                if let (Some(n), Some(i)) = (fname, fid) {
                    methods.push(json!({
                        "name": n, // Sera converti en snake_case par le template
                        "id": i,
                        "visibility": "pub", // Par défaut
                        "return_type": "Result<()>", // Par défaut
                        "description": format!("Implémentation de la fonction {}", n)
                    }));
                }
            }
        }

        // 3. Transformation des Sous-composants en Champs (Composition)
        let mut fields = Vec::new();
        // Check ownedLogicalComponents (LA) OR ownedSystemComponents (SA)
        let logical_children = element
            .get("ownedLogicalComponents")
            .and_then(|v| v.as_array());
        let system_children = element
            .get("ownedSystemComponents")
            .and_then(|v| v.as_array());

        // On fusionne les deux sources possibles (selon la couche)
        let all_children = logical_children
            .into_iter()
            .flatten()
            .chain(system_children.into_iter().flatten());

        for child in all_children {
            let cname = child.get("name").and_then(|v| v.as_str());
            let cid = child.get("id").and_then(|v| v.as_str());

            if let (Some(n), Some(i)) = (cname, cid) {
                fields.push(json!({
                    "name": n.to_lowercase(), // convention variable
                    "type": n, // convention type (Nom de la classe du composant)
                    "id": i,
                    "visibility": "private"
                }));
            }
        }

        // 4. Gestion de l'héritage
        let parent_class = element
            .get("base_class")
            .and_then(|obj| obj.get("name"))
            .and_then(|v| v.as_str());

        // 5. Construction de l'objet final pour le Template
        // Structure optimisée pour les générateurs Rust/C++/TS
        Ok(json!({
            "domain": "software",
            "meta": {
                "uuid": id,
                "source_element": name,
                "generated_at": chrono::Utc::now().to_rfc3339()
            },
            "entity": {
                "name": name,
                "kind": "class", // ou struct
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

    #[test]
    fn test_software_transformation_methods_and_fields() {
        let transformer = SoftwareTransformer;

        // Mock d'un composant hydraté (JSON pur)
        let component = json!({
            "id": "UUID_COMP_1",
            "name": "FlightController",
            // Fonctions allouées -> Méthodes
            "ownedFunctionalAllocation": [
                { "id": "UUID_F1", "name": "ComputeAltitude" },
                { "id": "UUID_F2", "name": "LogData" }
            ],
            // Sous-composants -> Champs
            "ownedLogicalComponents": [
                { "id": "UUID_SUB1", "name": "GPSModule" }
            ],
            // Héritage -> Parent
            "base_class": { "id": "UUID_PARENT", "name": "GenericController" }
        });

        let result = transformer
            .transform(&component)
            .expect("Transformation failed");

        // Vérifications structurelles
        assert_eq!(result["domain"], "software");
        assert_eq!(result["entity"]["name"], "FlightController");
        assert_eq!(result["entity"]["parent"], "GenericController");

        // Vérification des méthodes
        let methods = result["entity"]["methods"]
            .as_array()
            .expect("Methods missing");
        assert_eq!(methods.len(), 2);
        assert_eq!(methods[0]["name"], "ComputeAltitude");
        assert_eq!(methods[0]["visibility"], "pub");

        // Vérification des champs
        let fields = result["entity"]["fields"]
            .as_array()
            .expect("Fields missing");
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0]["type"], "GPSModule");
        assert_eq!(fields[0]["name"], "gpsmodule"); // lowercase check
    }

    #[test]
    fn test_software_transformation_minimal_element() {
        let transformer = SoftwareTransformer;
        // Test avec un élément vide pour vérifier la robustesse (pas de crash)
        let component = json!({ "id": "UUID_EMPTY", "name": "EmptyBox" });

        let result = transformer
            .transform(&component)
            .expect("Should handle minimal input");

        assert_eq!(result["entity"]["name"], "EmptyBox");
        assert!(result["entity"]["methods"].as_array().unwrap().is_empty());
        assert!(result["entity"]["fields"].as_array().unwrap().is_empty());
        assert!(result["entity"]["parent"].is_null());
    }
}
