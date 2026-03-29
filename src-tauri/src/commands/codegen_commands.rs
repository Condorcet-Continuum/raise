// FICHIER : src-tauri/src/commands/codegen_commands.rs

use crate::utils::prelude::*;

use crate::code_generator::CodeGeneratorService;
use crate::commands::rules_commands::RuleEngineState;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::transformers::{get_transformer, TransformationDomain};
use tauri::State;

/// Génère une représentation technique (Code, VHDL, Doc) pour un élément donné.
#[tauri::command]
pub async fn generate_source_code(
    element_id: String,
    domain: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<JsonValue> {
    // 1. Résolution du domaine de transformation
    let target_domain = match domain.to_lowercase().as_str() {
        "software" | "code" | "rust" | "cpp" => TransformationDomain::Software,
        "hardware" | "vhdl" | "fpga" | "verilog" => TransformationDomain::Hardware,
        "system" | "overview" | "doc" | "architecture" => TransformationDomain::System,
        _ => {
            raise_error!(
                "ERR_CODEGEN_DOMAIN_UNSUPPORTED",
                error = format!(
                    "Le domaine '{}' n'est pas supporté par le moteur de génération.",
                    domain
                )
            );
        }
    };

    // 2. Récupération du contexte Space/DB depuis le modèle en mémoire
    let (space, db) = {
        let model = state.model.lock().await;
        // Le nom du modèle est stocké sous la forme "workspace/database"
        let parts: Vec<&str> = model.meta.name.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            ("default".to_string(), "default".to_string())
        }
    };

    // 3. Initialisation et Indexation du Loader Dynamique
    let loader = ModelLoader::new(&storage, &space, &db);
    loader.index_project().await?;

    // 4. 🎯 RÉPARATION PURE GRAPH :
    // On récupère l'élément via 'get_element' (normalisé) puis on le convertit en JSON.
    let element = loader.get_element(&element_id).await?;
    let element_json = match json::serialize_to_value(&element) {
        Ok(v) => v,
        Err(e) => raise_error!(
            "ERR_CODEGEN_SERIALIZATION_FAILED",
            error = e,
            context = json_value!({ "element_id": element_id })
        ),
    };

    // 5. Exécution de la transformation via le transformer dédié
    let transformer = get_transformer(target_domain);

    // 🎯 FIX : Annotation de type explicite pour lever l'ambiguïté du compilateur
    let result: JsonValue = match transformer.transform(&element_json) {
        Ok(data) => data,
        Err(e) => raise_error!(
            "ERR_DATA_TRANSFORMATION_FAILED",
            error = e,
            context = json_value!({ "domain": domain })
        ),
    };

    Ok(result)
}

async fn get_active_space_and_db(state: &State<'_, RuleEngineState>) -> (String, String) {
    let model = state.model.lock().await;
    let parts: Vec<&str> = model.meta.name.split('/').collect();
    if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        ("default".to_string(), "default".to_string())
    }
}

/// Commande Front-End : Demande l'ingestion d'un fichier physique dans le Jumeau Numérique
#[tauri::command]
pub async fn ingest_code_file(
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<usize> {
    let (space, db) = get_active_space_and_db(&state).await;
    let manager = CollectionsManager::new(&storage, &space, &db);

    let service = CodeGeneratorService::new(PathBuf::from(""));
    let count = service.ingest_file(&PathBuf::from(path), &manager).await?;

    Ok(count)
}

/// Commande Front-End : Matérialise le Jumeau Numérique dans un fichier physique
#[tauri::command]
pub async fn weave_code_file(
    module_name: String,
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<String> {
    let (space, db) = get_active_space_and_db(&state).await;
    let manager = CollectionsManager::new(&storage, &space, &db);

    let service = CodeGeneratorService::new(PathBuf::from(""));
    let final_path = service
        .weave_file(&module_name, &PathBuf::from(path), &manager)
        .await?;

    Ok(final_path.to_string_lossy().to_string())
}
// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::AgentDbSandbox;

    /// Prépare le mapping ontologique pour le test
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
                    "search_spaces": [ { "layer": "la", "collection": "components" } ]
                }),
            )
            .await
            .unwrap();
    }

    #[async_test]
    async fn test_generate_code_logic_pure_graph() {
        let sandbox = AgentDbSandbox::new().await;
        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_mapping(&sys_mgr).await;

        // On injecte un composant dans la collection physique 'la/components'
        let la_mgr = CollectionsManager::new(&sandbox.db, &sandbox.config.system_domain, "la");
        let _ = la_mgr
            .create_collection(
                "components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await;

        let component_id = "UUID-COMP-TEST";
        let component = json_value!({
            "_id": component_id,
            "name": "RadarSystem",
            "type": "LogicalComponent",
            "properties": {
                "version": "1.2.0"
            }
        });
        la_mgr.insert_raw("components", &component).await.unwrap();

        // Initialisation du Loader
        let loader = ModelLoader::new_with_manager(sys_mgr);
        loader.index_project().await.unwrap();

        // 🎯 RÉPARATION TEST : On simule la logique de la commande
        let element = loader.get_element(component_id).await.unwrap();
        let element_json = json::serialize_to_value(&element).unwrap();

        let transformer = get_transformer(TransformationDomain::Software);
        let result = transformer
            .transform(&element_json)
            .expect("La transformation a échoué");

        // Assertions sur la structure de sortie attendue par le front-end
        assert_eq!(result["domain"], "software");
        assert_eq!(result["entity"]["name"], "RadarSystem");
    }
}
