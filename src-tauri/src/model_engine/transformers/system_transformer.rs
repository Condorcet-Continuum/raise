use super::ModelTransformer;
use anyhow::Result;
use serde_json::{json, Value};

pub struct SystemTransformer;

impl ModelTransformer for SystemTransformer {
    fn transform(&self, element: &Value) -> Result<Value> {
        let name = element
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("System");
        let id = element.get("id").and_then(|v| v.as_str()).unwrap_or("");

        // 1. Extraction des Acteurs (Intervenants externes)
        // Dans Arcadia, le système interagit avec des Acteurs via des allocations ou des échanges
        // Ici, on liste simplement les acteurs connectés si l'élément est le Système Racine
        let mut actors = Vec::new();
        // On suppose que l'élément passé est le "System Analysis Root" ou le "System Component"
        // Si c'est le System Component, on peut regarder ses connexions (non implémenté en détail ici)
        // Pour l'exemple, on regarde si des acteurs sont contenus (cas rare) ou référencés.

        // Note: Dans une implémentation réelle, on traverserait le graphe pour trouver les acteurs connectés.
        // Ici, on se base sur une propriété hypothétique "allocatedActors" ou on laisse vide pour le template.
        if let Some(allocated) = element.get("allocatedActors").and_then(|v| v.as_array()) {
            for actor in allocated {
                if let Some(aname) = actor.get("name").and_then(|v| v.as_str()) {
                    actors.push(json!({ "name": aname, "type": "ExternalActor" }));
                }
            }
        }

        // 2. Extraction des Capacités Système (Ce que le système doit faire)
        let mut capabilities = Vec::new();
        if let Some(caps) = element
            .get("ownedSystemCapability")
            .and_then(|v| v.as_array())
        {
            for cap in caps {
                if let Some(cname) = cap.get("name").and_then(|v| v.as_str()) {
                    let desc = cap
                        .get("description")
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
                if let Some(fname) = func.get("name").and_then(|v| v.as_str()) {
                    functions.push(json!({ "name": fname }));
                }
            }
        }

        // Structure optimisée pour Tera (Documentation / Vue d'ensemble)
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

    #[test]
    fn test_system_transformation() {
        let transformer = SystemTransformer;

        // Mock d'un élément System Analysis racine
        let system_element = json!({
            "id": "UUID_SYS_1",
            "name": "DroneSystem",
            // Capacités du système (System Capabilities)
            "ownedSystemCapability": [
                { "id": "CAP_1", "name": "Perform Autonomous Flight", "description": "Voler sans pilote" },
                { "id": "CAP_2", "name": "Video Surveillance" }
            ],
            // Fonctions racine
            "ownedSystemFunctions": [
                { "id": "FUNC_S1", "name": "Detect Obstacles" }
            ],
            // Acteurs (Simulés ici via une propriété directe pour le test)
            "allocatedActors": [
                { "id": "ACT_1", "name": "Operator" }
            ]
        });

        let result = transformer
            .transform(&system_element)
            .expect("Transformation failed");

        assert_eq!(result["domain"], "system");
        assert_eq!(result["system_overview"]["name"], "DroneSystem");

        // Vérification Capabilities
        let caps = result["system_overview"]["capabilities"]
            .as_array()
            .expect("Capabilities missing");
        assert_eq!(caps.len(), 2);
        assert_eq!(caps[0]["name"], "Perform Autonomous Flight");
        assert_eq!(caps[0]["description"], "Voler sans pilote");

        // Vérification Fonctions
        let funcs = result["system_overview"]["root_functions"]
            .as_array()
            .expect("Functions missing");
        assert_eq!(funcs.len(), 1);
        assert_eq!(funcs[0]["name"], "Detect Obstacles");

        // Vérification Acteurs
        let actors = result["system_overview"]["actors"]
            .as_array()
            .expect("Actors missing");
        assert_eq!(actors.len(), 1);
        assert_eq!(actors[0]["name"], "Operator");
    }
}
