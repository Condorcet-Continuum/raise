// FICHIER : src-tauri/src/commands/codegen_commands.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::code_generator::CodeGeneratorService;
use crate::commands::rules_commands::RuleEngineState;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::transformers::{get_transformer, TransformationDomain};
use crate::model_engine::types::ProjectModel;
use tauri::State;

/// Génère une représentation technique (Code, VHDL, Doc) pour un élément donné.
#[tauri::command]
pub async fn generate_source_code(
    element_id: String,
    domain: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<JsonValue> {
    let model_guard = state.model.lock().await;

    // 1. Résolution du domaine de transformation via Match strict
    let target_domain = match domain.to_lowercase().as_str() {
        "software" | "code" | "rust" | "cpp" => TransformationDomain::Software,
        "hardware" | "vhdl" | "fpga" | "verilog" => TransformationDomain::Hardware,
        "system" | "overview" | "doc" | "architecture" => TransformationDomain::System,
        _ => {
            raise_error!(
                "ERR_CODEGEN_DOMAIN_UNSUPPORTED",
                error = format!("Le domaine '{}' n'est pas supporté.", domain)
            );
        }
    };

    // 2. Récupération résiliente du contexte Space/DB
    let (space, db) = resolve_active_context(&model_guard);

    // 3. Initialisation et Indexation du Loader Dynamique
    let loader = ModelLoader::new(&storage, &space, &db);
    if let Err(e) = loader.index_project().await {
        raise_error!("ERR_CODEGEN_INDEX_FAILED", error = e.to_string());
    }

    // 4. Extraction et sérialisation
    let element = loader.get_element(&element_id).await?;
    let element_json = match json::serialize_to_value(&element) {
        Ok(v) => v,
        Err(e) => raise_error!("ERR_CODEGEN_SERIALIZATION_FAILED", error = e.to_string()),
    };

    // 5. Exécution de la transformation sémantique
    let transformer = get_transformer(target_domain);
    match transformer.transform(&element_json) {
        Ok(result) => Ok(result),
        Err(e) => raise_error!("ERR_DATA_TRANSFORMATION_FAILED", error = e.to_string()),
    }
}

/// Résout l'espace et la base de données à partir du jumeau numérique en mémoire.
fn resolve_active_context(model: &ProjectModel) -> (String, String) {
    let config = AppConfig::get();
    let parts: Vec<&str> = model.meta.name.split('/').collect();

    if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        // Fallback sur les Mount Points système configurés (SSOT)
        (
            config.mount_points.system.domain.clone(),
            config.mount_points.system.db.clone(),
        )
    }
}

/// Commande Front-End : Ingestion d'un fichier physique.
#[tauri::command]
pub async fn ingest_code_file(
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<usize> {
    let model_guard = state.model.lock().await;
    let (space, db) = resolve_active_context(&model_guard);
    let manager = CollectionsManager::new(&storage, &space, &db);

    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();
    let service = CodeGeneratorService::new(domain_root);

    let prod_schema_uri = "db://_system/_system/schemas/v1/dapps/services/code_element.schema.json";
    match service
        .ingest_file(&PathBuf::from(path), &manager, prod_schema_uri)
        .await
    {
        Ok(count) => Ok(count),
        Err(e) => raise_error!("ERR_CODEGEN_INGESTION_FAILED", error = e.to_string()),
    }
}

/// Commande Front-End : Matérialise le Jumeau Numérique sur le disque.
#[tauri::command]
pub async fn weave_code_file(
    module_name: String,
    path: String,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<String> {
    let model_guard = state.model.lock().await;
    let (space, db) = resolve_active_context(&model_guard);
    let manager = CollectionsManager::new(&storage, &space, &db);

    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();
    let service = CodeGeneratorService::new(domain_root);

    match service
        .weave_file(&module_name, &PathBuf::from(path), &manager)
        .await
    {
        Ok(final_path) => Ok(final_path.to_string_lossy().to_string()),
        Err(e) => raise_error!("ERR_CODEGEN_WEAVE_FAILED", error = e.to_string()),
    }
}

// =========================================================================
// TESTS UNITAIRES (Sans anti-pattern Default)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    async fn inject_mock_mapping(manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            manager.space, manager.db
        );
        manager
            .create_collection("configs", &generic_schema)
            .await?;
        manager
            .upsert_document(
                "configs",
                json_value!({
                    "_id": "ref:configs:handle:ontological_mapping",
                    "search_spaces": [ { "layer": "la", "collection": "components" } ]
                }),
            )
            .await?;
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_generate_code_logic_pure_graph() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        inject_mock_mapping(&sys_mgr).await?;

        let la_mgr = CollectionsManager::new(&sandbox.db, &config.mount_points.system.domain, "la");
        AgentDbSandbox::mock_db(&la_mgr).await?;

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        la_mgr.create_collection("components", &schema_uri).await?;

        let component_id = "UUID-COMP-TEST";
        la_mgr.insert_raw("components", &json_value!({
            "_id": component_id, "handle": component_id, "name": "RadarSystem", "type": "LogicalComponent"
        })).await?;

        let loader = ModelLoader::new_with_manager(sys_mgr);
        loader.index_project().await?;

        let element = loader.get_element(component_id).await?;
        let element_json = json::serialize_to_value(&element).unwrap();

        let transformer = get_transformer(TransformationDomain::Software);
        let result = transformer
            .transform(&element_json)
            .expect("Transformation failed");

        assert_eq!(result["domain"], "software");
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_context_resolution_resilience() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await;
        let mut model = ProjectModel::default();
        model.meta.name = "workspace_a/db_b".to_string();

        let (space, db) = resolve_active_context(&model);
        assert_eq!(space, "workspace_a");
        assert_eq!(db, "db_b");
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_context_resolution_fallback() -> RaiseResult<()> {
        let _sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let model = ProjectModel::default(); // Nom vide

        let (space, db) = resolve_active_context(&model);
        assert_eq!(space, config.mount_points.system.domain);
        assert_eq!(db, config.mount_points.system.db);
        Ok(())
    }
}
