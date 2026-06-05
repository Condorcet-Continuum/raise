// FICHIER : crates/raise-core/src/services/codegen_service.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::code_generator::CodeGeneratorService;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::transformers::{get_transformer, TransformationDomain};
use crate::model_engine::types::ProjectModel;

// 🎯 Fonction rendue publique pour que Tauri puisse l'utiliser
pub fn resolve_active_context(model: &ProjectModel) -> (String, String) {
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

pub async fn generate_source_code(
    element_id: &str,
    target_domain_str: &str, // 🎯 L'axe de transformation (ex: "software")
    domain: &str,            // 🎯 L'espace de travail MBSE (ex: "_system")
    db: &str,
    storage: &StorageEngine,
) -> RaiseResult<JsonValue> {
    let target_domain = match target_domain_str.to_lowercase().as_str() {
        "software" | "code" | "rust" | "cpp" => TransformationDomain::Software,
        "hardware" | "vhdl" | "fpga" | "verilog" => TransformationDomain::Hardware,
        "system" | "overview" | "doc" | "architecture" => TransformationDomain::System,
        _ => {
            raise_error!(
                "ERR_CODEGEN_DOMAIN_UNSUPPORTED",
                error = format!(
                    "Le domaine cible '{}' n'est pas supporté.",
                    target_domain_str
                )
            );
        }
    };

    // 🎯 FIX : On utilise ici le "domain" (l'espace) pour pointer vers la bonne base
    let loader = ModelLoader::new(storage, domain, db)?;
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

pub async fn auto_tag_module(
    module_handle: &str,
    domain: &str,
    db: &str,
    storage: &StorageEngine,
) -> RaiseResult<usize> {
    let manager = CollectionsManager::new(storage, domain, db);
    let module_doc = match manager.get_document("modules", module_handle).await {
        Ok(Some(doc)) => doc,
        Ok(None) => raise_error!(
            "ERR_CODEGEN_MODULE_NOT_FOUND",
            error = format!("Le module '{}' est introuvable.", module_handle)
        ),
        Err(e) => raise_error!("ERR_CODEGEN_MODULE_DB_ERROR", error = e.to_string()),
    };

    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();
    let service = CodeGeneratorService::new(domain_root, &manager).await?;

    // 🎯 Appel délégué au service étendu
    service.auto_tag_module(module_doc).await
}

pub async fn ingest_module(
    module_handle: &str,
    domain: &str,
    db: &str,
    storage: &StorageEngine,
    is_test_mode: bool,
) -> RaiseResult<usize> {
    let manager = CollectionsManager::new(storage, domain, db);
    let module_doc = match manager.get_document("modules", module_handle).await {
        Ok(Some(doc)) => doc,
        Ok(None) => raise_error!(
            "ERR_CODEGEN_MODULE_NOT_FOUND",
            error = format!("Le module '{}' est introuvable.", module_handle)
        ),
        Err(e) => raise_error!("ERR_CODEGEN_MODULE_DB_ERROR", error = e.to_string()),
    };

    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();
    let mut service = CodeGeneratorService::new(domain_root, &manager).await?;
    if is_test_mode {
        service = service.with_test_mode();
    }

    // 🎯 Appel délégué au service étendu
    service.ingest_module(module_doc, &manager).await
}

pub async fn weave_module(
    module_handle: &str,
    domain: &str,
    db: &str,
    storage: &StorageEngine,
    is_test_mode: bool,
) -> RaiseResult<String> {
    let manager = CollectionsManager::new(storage, domain, db);
    let module_doc = match manager.get_document("modules", module_handle).await {
        Ok(Some(doc)) => doc,
        Ok(None) => raise_error!(
            "ERR_CODEGEN_MODULE_NOT_FOUND",
            error = format!("Le module '{}' est introuvable.", module_handle)
        ),
        Err(e) => raise_error!("ERR_CODEGEN_MODULE_DB_ERROR", error = e.to_string()),
    };

    let domain_root = AppConfig::get()
        .get_path("PATH_RAISE_DOMAIN")
        .unwrap_or_default();
    let mut service = CodeGeneratorService::new(domain_root, &manager).await?;
    if is_test_mode {
        service = service.with_test_mode();
    }

    // 🎯 Appel délégué au service étendu
    match service.weave_module(module_doc, &manager).await {
        Ok(final_path) => Ok(final_path.to_string_lossy().to_string()),
        Err(e) => raise_error!("ERR_CODEGEN_WEAVE_FAILED", error = e.to_string()),
    }
}

pub async fn link_module(
    module_handle: &str, // 🎯 L'argument restreignant l'analyse
    domain: &str,
    db: &str,
    storage: &StorageEngine,
) -> RaiseResult<usize> {
    use crate::code_generator::analyzers::dependency_analyzer::DependencyAnalyzer;

    let manager = CollectionsManager::new(storage, domain, db);
    let analyzer = DependencyAnalyzer::new(&manager);

    let resolved_count = analyzer.link_module("code_elements", module_handle).await?;

    Ok(resolved_count)
}
