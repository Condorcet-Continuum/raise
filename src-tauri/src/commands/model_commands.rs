// FICHIER : src-tauri/src/commands/model_commands.rs

use crate::utils::prelude::*;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ProjectModel;
use tauri::{command, State};

/// Charge l'intégralité du modèle en mémoire pour analyse.
#[command]
pub async fn load_project_model(
    storage: State<'_, StorageEngine>,
    space: String,
    db: String,
) -> RaiseResult<ProjectModel> {
    let loader = ModelLoader::from_engine(storage.inner(), &space, &db);

    match loader.load_full_model().await {
        Ok(model) => Ok(model),
        Err(e) => raise_error!(
            "ERR_MODEL_LOAD_FAIL",
            error = e,
            context = json_value!({
                "action": "load_full_project_model",
                "space": space,
                "db": db
            })
        ),
    }
}

/// 📥 Ingeste un tableau JSON généré par l'IA dans le Graphe Arcadia
/// Utilise le mapping ontologique pour router, en s'appuyant sur le Schéma JSON strict
pub async fn ingest_arcadia_elements(
    storage: &StorageEngine,
    domain: &str,
    sys_db: &str,
    json_output: &str,
) -> RaiseResult<Vec<String>> {
    let parsed_json = match json::deserialize_from_str::<JsonValue>(json_output) {
        Ok(j) => j,
        Err(e) => raise_error!("ERR_JSON_PARSE", error = e.to_string()),
    };

    // 🎯 FIX : Tolérance maximale (Accepte un Array direct OU un Array dans un Objet)
    let elements = if let Some(arr) = parsed_json.as_array() {
        arr.clone()
    } else if let Some(obj) = parsed_json.as_object() {
        // L'IA a wrappé le tableau dans un objet (ex: {"elements": [...]})
        let found_array = obj
            .get("elements")
            .and_then(|v| v.as_array())
            .or_else(|| obj.values().find_map(|v| v.as_array())); // Cherche n'importe quel tableau

        match found_array {
            Some(arr) => arr.clone(),
            None => raise_error!(
                "ERR_FORMAT",
                error = "Aucun tableau d'éléments trouvé dans l'objet JSON."
            ),
        }
    } else {
        raise_error!(
            "ERR_FORMAT",
            error = "Le LLM n'a pas renvoyé un format reconnu."
        );
    };

    let sys_mgr = CollectionsManager::new(storage, domain, sys_db);
    let mapping_doc = sys_mgr
        .get_document("configs", "ref:configs:handle:ontological_mapping")
        .await?
        .unwrap_or_default();

    // ... [Le reste de la fonction (la boucle for el in elements) reste STRICTEMENT IDENTIQUE] ...
    let mappings = match mapping_doc.get("mappings").and_then(|v| v.as_object()) {
        Some(m) => m,
        None => raise_error!(
            "ERR_MAPPING_MISSING",
            error = "Aucun 'mappings' trouvé dans ontological_mapping."
        ),
    };

    let mut ingested_ids = Vec::new();

    for el in &elements {
        // Note: on itère sur la référence
        let doc = el.clone();
        let kind = doc
            .get("@type")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown")
            .to_string();

        if let Some(mapping) = mappings.get(&kind) {
            let target_layer = mapping["layer"].as_str().unwrap_or(sys_db);
            let target_collection = mapping["collection"].as_str().unwrap();

            let target_mgr = CollectionsManager::new(storage, domain, target_layer);

            match target_mgr.upsert_document(target_collection, doc).await {
                Ok(res) => ingested_ids.push(res),
                Err(e) => user_warn!(
                    "WRN_INGESTION_FAILED",
                    json_value!({"error": e.to_string(), "kind": kind})
                ),
            }
        } else {
            user_warn!("WRN_UNKNOWN_ONTOLOGY_KIND", json_value!({"kind": kind}));
        }
    }

    Ok(ingested_ids)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    // 🎯 FIX : Ajout de DbSandbox pour pouvoir initialiser physiquement la base cible
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    /// Helper pour satisfaire l'exigence Data-Driven du Loader et du Routeur
    async fn inject_mock_mapping(manager: &CollectionsManager<'_>) {
        let _ = manager
            .create_collection(
                "configs",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;
        manager
            .upsert_document(
                "configs",
                json_value!({
                    "_id": "ref:configs:handle:ontological_mapping",
                    "mappings": {
                        "OperationalActor": { "layer": "oa", "collection": "actors" },
                        "EnvironmentalConstraint": { "layer": "oa", "collection": "constraints" }
                    },
                    "search_spaces": [ { "layer": "oa", "collection": "actors" } ]
                }),
            )
            .await
            .unwrap();
    }

    #[async_test]
    async fn test_load_project_model_command() {
        let sandbox = AgentDbSandbox::new().await;
        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        DbSandbox::mock_db(&sys_mgr).await.unwrap();
        inject_mock_mapping(&sys_mgr).await;

        let space = sandbox.config.system_domain.clone();
        let db = sandbox.config.system_db.clone();
        let loader = ModelLoader::from_engine(&sandbox.db, &space, &db);

        let result = loader.load_full_model().await;
        assert!(result.is_ok());
        let model = result.unwrap();
        assert_eq!(model.meta.element_count, 0);
    }

    // 🎯 NOUVEAU TEST : Vérification de l'ingestion et du routage sémantique
    #[async_test]
    async fn test_ingest_arcadia_elements_success() {
        let sandbox = AgentDbSandbox::new().await;
        let domain = &sandbox.config.system_domain;
        let sys_db = &sandbox.config.system_db;

        // 1. Setup du Système (Mapping Ontologique)
        let sys_mgr = CollectionsManager::new(&sandbox.db, domain, sys_db);
        DbSandbox::mock_db(&sys_mgr).await.unwrap();
        inject_mock_mapping(&sys_mgr).await;

        // 2. Setup de la base cible (La couche 'oa' avec sa collection 'actors')
        let target_mgr = CollectionsManager::new(&sandbox.db, domain, "oa");
        DbSandbox::mock_db(&target_mgr).await.unwrap();
        target_mgr
            .create_collection(
                "actors",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        // 3. Simulation de la réponse parfaite du LLM (Pilotée par notre Schéma JSON)
        let llm_json_output = r#"[
            {
                "handle": "astronaut",
                "name": "Astronaute",
                "@type": "OperationalActor",
                "description": "Pilote du rover sur la surface lunaire."
            },
            {
                "handle": "station",
                "name": "Station Orbitale",
                "@type": "OperationalActor",
                "description": "Supervise les opérations."
            }
        ]"#;

        // 4. Exécution de l'ingestion
        let result = ingest_arcadia_elements(&sandbox.db, domain, sys_db, llm_json_output).await;

        // 5. Assertions sur le retour de la fonction
        assert!(result.is_ok(), "L'ingestion a échoué : {:?}", result.err());
        let ingested_ids = result.unwrap();
        assert_eq!(
            ingested_ids.len(),
            2,
            "Deux éléments auraient dû être ingérés."
        );

        // 6. Assertions Physiques (Vérification dans la base de données cible 'oa/actors')
        let doc1 = target_mgr
            .get_document("actors", "astronaut")
            .await
            .unwrap();
        assert!(
            doc1.is_some(),
            "L'astronaute n'a pas été sauvegardé physiquement !"
        );

        let doc1_val = doc1.unwrap();
        assert_eq!(doc1_val["name"].as_str().unwrap(), "Astronaute");
        assert_eq!(doc1_val["@type"].as_str().unwrap(), "OperationalActor");
        assert_eq!(
            doc1_val["description"].as_str().unwrap(),
            "Pilote du rover sur la surface lunaire."
        );
    }
}
