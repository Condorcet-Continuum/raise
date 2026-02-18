// FICHIER : src-tauri/src/model_engine/transformers/system_transformer.rs

use crate::utils::prelude::*;

use super::ModelTransformer;
use crate::model_engine::arcadia; // <-- Vocabulaire

pub struct SystemTransformer;

impl ModelTransformer for SystemTransformer {
    fn transform(&self, element: &Value) -> Result<Value> {
        let name = element
            .get(arcadia::PROP_NAME)
            .and_then(|v| v.as_str())
            .unwrap_or("System");

        let id = element
            .get(arcadia::PROP_ID)
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 1. Extraction des Acteurs
        let mut actors = Vec::new();
        if let Some(allocated) = element.get("allocatedActors").and_then(|v| v.as_array()) {
            for actor in allocated {
                if let Some(aname) = actor.get(arcadia::PROP_NAME).and_then(|v| v.as_str()) {
                    actors.push(json!({ "name": aname, "type": "ExternalActor" }));
                }
            }
        }

        // 2. Extraction des Capacités Système
        let mut capabilities = Vec::new();
        if let Some(caps) = element
            .get("ownedSystemCapability")
            .and_then(|v| v.as_array())
        {
            for cap in caps {
                if let Some(cname) = cap.get(arcadia::PROP_NAME).and_then(|v| v.as_str()) {
                    let desc = cap
                        .get(arcadia::PROP_DESCRIPTION)
                        .and_then(|v| v.as_str())
                        .unwrap_or("");

                    capabilities.push(json!({
                        "name": cname,
                        "description": desc
                    }));
                }
            }
        }

        // 3. Fonctions Système de haut niveau
        let mut functions = Vec::new();
        if let Some(funcs) = element
            .get("ownedSystemFunctions")
            .and_then(|v| v.as_array())
        {
            for func in funcs {
                if let Some(fname) = func.get(arcadia::PROP_NAME).and_then(|v| v.as_str()) {
                    functions.push(json!({ "name": fname }));
                }
            }
        }

        // Structure optimisée pour Tera
        Ok(json!({
            "domain": "system",
            "meta": {
                "uuid": id,
                "project_name": name,
                "generated_at": chrono::Utc::now().to_rfc3339()
            },
            "system_overview": {
                "name": name,
                "actors": actors,
                "capabilities": capabilities,
                "root_functions": functions
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia;

    #[test]
    fn test_system_transformation() {
        let transformer = SystemTransformer;

        let system_element = json!({
            arcadia::PROP_ID: "UUID_SYS_1",
            arcadia::PROP_NAME: "DroneSystem",
            "ownedSystemCapability": [
                {
                    arcadia::PROP_ID: "CAP_1",
                    arcadia::PROP_NAME: "Perform Autonomous Flight",
                    arcadia::PROP_DESCRIPTION: "Voler sans pilote"
                },
                { arcadia::PROP_ID: "CAP_2", arcadia::PROP_NAME: "Video Surveillance" }
            ],
            "ownedSystemFunctions": [
                { arcadia::PROP_ID: "FUNC_S1", arcadia::PROP_NAME: "Detect Obstacles" }
            ],
            "allocatedActors": [
                { arcadia::PROP_ID: "ACT_1", arcadia::PROP_NAME: "Operator" }
            ]
        });

        let result = transformer
            .transform(&system_element)
            .expect("Transformation failed");

        assert_eq!(result["domain"], "system");
        assert_eq!(result["system_overview"]["name"], "DroneSystem");

        let caps = result["system_overview"]["capabilities"]
            .as_array()
            .unwrap();
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0]["description"], "Voler sans pilote");
    }
}
