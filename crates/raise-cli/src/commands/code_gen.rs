// FICHIER : crates/raise-cli/src/commands/code_gen.rs

use clap::{Args, Subcommand, ValueEnum};
use raise_core::{user_info, user_success, utils::prelude::*};

// 🎯 Imports sémantiques depuis la forge logicielle
use raise_core::code_generator::models::TargetLanguage;
use raise_core::code_generator::CodeGeneratorService;
use raise_core::json_db::collections::manager::CollectionsManager;

use crate::CliContext;

#[derive(Args, Clone, Debug)]
pub struct CodeGenArgs {
    #[command(subcommand)]
    pub command: CodeGenCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum CodeGenCommands {
    Generate {
        element_id: String,
        #[arg(short, long, value_enum)]
        lang: CliTargetLanguage,
        #[arg(short, long)]
        out_dir: Option<String>,
    },
    Ingest {
        path: String,
    },
    Weave {
        module_name: String,
        path: String,
    },
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum CliTargetLanguage {
    Rust,
    Typescript,
    Cpp,
    Verilog,
    Vhdl,
    Python,
}

impl From<CliTargetLanguage> for TargetLanguage {
    fn from(lang: CliTargetLanguage) -> Self {
        match lang {
            CliTargetLanguage::Rust => TargetLanguage::Rust,
            CliTargetLanguage::Typescript => TargetLanguage::TypeScript,
            CliTargetLanguage::Cpp => TargetLanguage::Cpp,
            CliTargetLanguage::Verilog => TargetLanguage::Verilog,
            CliTargetLanguage::Vhdl => TargetLanguage::Vhdl,
            CliTargetLanguage::Python => TargetLanguage::Python,
        }
    }
}

pub async fn handle(args: CodeGenArgs, ctx: CliContext) -> RaiseResult<()> {
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        CodeGenCommands::Generate {
            element_id,
            lang,
            out_dir,
        } => {
            let target: TargetLanguage = lang.into();
            let target_dir = PathBuf::from(out_dir.unwrap_or_else(|| ".".to_string()));

            user_info!(
                "FORGE_GENERATE_INIT",
                json_value!({
                    "element_id": element_id,
                    "language": format!("{:?}", target),
                    "domain": ctx.active_domain,
                    "target_dir": target_dir.display().to_string()
                })
            );

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let mut service = CodeGeneratorService::new(target_dir, &manager).await?;

            if ctx.is_test_mode {
                service = service.with_test_mode();
            }

            match service.generate(&element_id, &manager, target).await {
                Ok(final_path) => user_success!(
                    "FORGE_SUCCESS",
                    json_value!({
                        "element": element_id,
                        "target": format!("{:?}", target),
                        "path": final_path.to_string_lossy()
                    })
                ),
                Err(e) => raise_error!(
                    "ERR_FORGE_GENERATE_FAILED",
                    error = e,
                    context = json_value!({"element_id": element_id})
                ),
            }
        }

        CodeGenCommands::Ingest { path } => {
            user_info!("CODE_INGEST_START", json_value!({ "path": path }));

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let mut service = CodeGeneratorService::new(PathBuf::from(""), &manager).await?;
            if ctx.is_test_mode {
                service = service.with_test_mode();
            }

            // 🎯 L'URI du schéma est désormais déduite en interne par Zéro Dette
            match service.ingest_file(&PathBuf::from(&path), &manager).await {
                Ok(count) => user_success!(
                    "CODE_INGEST_SUCCESS",
                    json_value!({ "path": path, "elements_ingested": count })
                ),
                Err(e) => raise_error!(
                    "ERR_CODE_INGEST_FAILED",
                    error = e,
                    context = json_value!({"path": path})
                ),
            }
        }

        CodeGenCommands::Weave { module_name, path } => {
            user_info!("CODE_WEAVE_START", json_value!({ "module": module_name }));

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let mut service = CodeGeneratorService::new(PathBuf::from(""), &manager).await?;
            if ctx.is_test_mode {
                service = service.with_test_mode();
            }

            match service
                .weave_file(&module_name, &PathBuf::from(&path), &manager)
                .await
            {
                Ok(final_path) => user_success!(
                    "CODE_WEAVE_SUCCESS",
                    json_value!({ "module": module_name, "final_path": final_path.to_string_lossy() })
                ),
                Err(e) => raise_error!(
                    "ERR_CODE_WEAVE_FAILED",
                    error = e,
                    context = json_value!({"module": module_name})
                ),
            }
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES (Conformité & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliContext;
    use raise_core::json_db::query::{Query, QueryEngine};
    use raise_core::json_db::storage::StorageEngine;
    use raise_core::utils::context::SessionManager;
    use raise_core::utils::testing::DbSandbox;

    /// 🎯 FIX : Injection stricte dans la partition _system, peu importe le domaine actif
    async fn inject_mock_codegen_config(storage: &SharedRef<StorageEngine>) -> RaiseResult<()> {
        let config = AppConfig::get();
        let sys_manager = CollectionsManager::new(
            storage.as_ref(),
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let generic_schema = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        let _ = DbSandbox::mock_db(&sys_manager).await;

        let _ = sys_manager
            .create_collection("components", &generic_schema)
            .await;
        let _ = sys_manager
            .create_collection("service_configs", &generic_schema)
            .await;

        sys_manager.upsert_document("components", json_value!({ "_id": "ref:components:handle:codegen_engine", "handle": "codegen_engine" })).await?;
        sys_manager.upsert_document("service_configs", json_value!({
            "_id": "mock_codegen",
            "component_id": "ref:components:handle:codegen_engine",
            "service_settings": {
                "format_on_save": true,
                "strict_mode": true,
                "semantic_routing": {
                    "software": { "aliases": ["rust", "cpp", "ts"], "collection": "code_elements", "schema_uri": generic_schema.clone() },
                    "doc": { "aliases": ["md"], "collection": "doc_elements", "schema_uri": generic_schema.clone() }
                }
            }
        })).await?;
        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_codegen_cli_dispatch() -> RaiseResult<()> {
        use raise_core::code_generator::models::{CodeElement, CodeElementType, Visibility};

        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage.clone());
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        // 🎯 FIX : On injecte directement via le moteur de stockage global
        inject_mock_codegen_config(&ctx.storage).await?;

        // Création de la collection de travail pour le domaine actif
        let _ = DbSandbox::mock_db(&manager).await;
        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let mock_el = CodeElement {
            module_id: None,
            parent_id: None,
            attributes: vec![],
            docs: None,
            elements: vec![],
            handle: "sa:Processor_A".to_string(),
            element_type: CodeElementType::Function,
            visibility: Visibility::Public,
            signature: "pub fn processor_a()".to_string(),
            body: Some("{}".to_string()),
            dependencies: vec![],
            metadata: UnorderedMap::new(),
        };
        let json_el = json::serialize_to_value(&mock_el).unwrap();
        manager.upsert_document("code_elements", json_el).await?;

        let test_out_dir = sandbox
            .storage
            .config
            .data_root
            .to_string_lossy()
            .to_string();

        let args = CodeGenArgs {
            command: CodeGenCommands::Generate {
                element_id: "sa:Processor_A".into(),
                lang: CliTargetLanguage::Rust,
                out_dir: Some(test_out_dir),
            },
        };

        handle(args, ctx).await
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_cli_ingest_and_weave_full_cycle() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;

        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage.clone());
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        // 🎯 FIX : On injecte directement via le moteur de stockage global
        inject_mock_codegen_config(&ctx.storage).await?;

        let _ = DbSandbox::mock_db(&manager).await;
        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        let initial_code = "// @raise-handle: fn:test_fn\npub fn test_fn() { }";
        fs::write_sync(&file_path, initial_code)
            .map_err(|e| build_error!("ERR_TEST_FS", error = e))?;

        let args_ingest = CodeGenArgs {
            command: CodeGenCommands::Ingest {
                path: file_path.to_string_lossy().to_string(),
            },
        };
        handle(args_ingest, ctx.clone()).await?;

        let query = Query::new("code_elements");
        let db_result = QueryEngine::new(&manager).execute_query(query).await?;

        if db_result.documents.is_empty() {
            raise_error!(
                "ERR_TEST_EMPTY_DB",
                error = "L'ingestion n'a créé aucun document."
            );
        }

        let mut doc = db_result.documents[0].clone();
        doc["body"] = json_value!("{ println!(\"RAISE_FORGE_OK\"); }");
        manager.upsert_document("code_elements", doc).await?;

        let args_weave = CodeGenArgs {
            command: CodeGenCommands::Weave {
                module_name: "test_weave".to_string(),
                path: file_path.to_string_lossy().to_string(),
            },
        };
        handle(args_weave, ctx.clone()).await?;

        let final_code = fs::read_to_string_sync(&file_path)
            .map_err(|e| build_error!("ERR_TEST_FS", error = e))?;
        if !final_code.contains("RAISE_FORGE_OK") {
            raise_error!(
                "ERR_TEST_FORGE_FAIL",
                error = "Le tissage du code a échoué."
            );
        }

        Ok(())
    }
}
