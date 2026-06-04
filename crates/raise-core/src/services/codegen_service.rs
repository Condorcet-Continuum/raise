// FICHIER : crates/raise-core/src/services/codegen_service.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::code_generator::CodeGeneratorService;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::transformers::{get_transformer, TransformationDomain};
use crate::model_engine::types::ProjectModel;
use crate::services::rules_service::RuleEngineState;

pub async fn generate_source_code(
    element_id: &str,
    domain: &str,
    rule_engine: &RuleEngineState,
    storage: &StorageEngine,
) -> RaiseResult<JsonValue> {
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

    let (space, db) = {
        let model_guard = rule_engine.model.lock().await;
        resolve_active_context(&model_guard)
    };

    let loader = ModelLoader::new(storage, &space, &db)?;
    if let Err(e) = loader.index_project().await {
        raise_error!("ERR_CODEGEN_INDEX_FAILED", error = e.to_string());
    }

    let element = loader.get_element(element_id).await?;
    let element_json = match json::serialize_to_value(&element) {
        Ok(v) => v,
        Err(e) => raise_error!("ERR_CODEGEN_SERIALIZATION_FAILED", error = e.to_string()),
    };

    let transformer = get_transformer(target_domain);
    match transformer.transform(&element_json) {
        Ok(result) => Ok(result),
        Err(e) => raise_error!("ERR_DATA_TRANSFORMATION_FAILED", error = e.to_string()),
    }
}

fn resolve_active_context(model: &ProjectModel) -> (String, String) {
    let config = AppConfig::get();
    let parts: Vec<&str> = model.meta.name.split('/').collect();

    if parts.len() >= 2 {
        (parts[0].to_string(), parts[1].to_string())
    } else {
        (
            config.mount_points.system.domain.clone(),
            config.mount_points.system.db.clone(),
        )
    }
}

pub async fn ingest_code_file(
    path: &str,
    rule_engine: &RuleEngineState,
    storage: &StorageEngine,
    is_test_mode: bool,
) -> RaiseResult<usize> {
    let (space, db) = {
        let model_guard = rule_engine.model.lock().await;
        resolve_active_context(&model_guard)
    };

    let manager = CollectionsManager::new(storage, &space, &db);
    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();

    // 🎯 INITIALISATION SÉCURISÉE (Avec le catalogue)
    let mut service = CodeGeneratorService::new(domain_root, &manager).await?;
    if is_test_mode {
        service = service.with_test_mode();
    }

    match service.ingest_file(Path::new(path), &manager).await {
        Ok(count) => Ok(count),
        Err(e) => raise_error!("ERR_CODEGEN_INGESTION_FAILED", error = e.to_string()),
    }
}

pub async fn weave_code_file(
    module_name: &str,
    path: &str,
    rule_engine: &RuleEngineState,
    storage: &StorageEngine,
    is_test_mode: bool,
) -> RaiseResult<String> {
    let (space, db) = {
        let model_guard = rule_engine.model.lock().await;
        resolve_active_context(&model_guard)
    };

    let manager = CollectionsManager::new(storage, &space, &db);
    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();

    let mut service = CodeGeneratorService::new(domain_root, &manager).await?;
    if is_test_mode {
        service = service.with_test_mode();
    }

    match service
        .weave_file(module_name, Path::new(path), &manager)
        .await
    {
        Ok(final_path) => Ok(final_path.to_string_lossy().to_string()),
        Err(e) => raise_error!("ERR_CODEGEN_WEAVE_FAILED", error = e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    async fn inject_mock_mapping(manager: &CollectionsManager<'_>) -> RaiseResult<()> {
        let config = AppConfig::get();
        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        manager
            .create_collection("configs", &generic_schema)
            .await?;
        manager.upsert_document("configs", json_value!({
            "_id": "ref:configs:handle:ontological_mapping", "search_spaces": [ { "layer": "la", "collection": "components" } ]
        })).await?;
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_generate_code_logic_pure_graph() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        inject_mock_mapping(&sys_mgr).await?;

        let la_mgr = CollectionsManager::new(&sandbox.db, &config.mount_points.system.domain, "la");
        DbSandbox::mock_db(&la_mgr).await?;

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        la_mgr.create_collection("components", &schema_uri).await?;

        let component_id = "UUID-COMP-TEST";
        la_mgr.insert_raw("components", &json_value!({
            "_id": component_id, "handle": component_id, "name": "RadarSystem", "type": "LogicalComponent"
        })).await?;

        let loader = ModelLoader::new_with_manager(sys_mgr)?;
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
        let _sandbox = AgentDbSandbox::new().await?;
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
        let _sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let model = ProjectModel::default(); // Nom vide

        let (space, db) = resolve_active_context(&model);
        assert_eq!(space, config.mount_points.system.domain);
        assert_eq!(db, config.mount_points.system.db);
        Ok(())
    }
}
