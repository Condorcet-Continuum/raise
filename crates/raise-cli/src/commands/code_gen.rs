// FICHIER : crates/raise-cli/src/commands/code_gen.rs

use clap::{Args, Subcommand, ValueEnum};
use raise_core::{user_info, user_success, utils::prelude::*};

// 🎯 Imports sémantiques depuis la forge logicielle
use crate::CliContext;
use raise_core::code_generator::models::TargetLanguage;
use raise_core::services::codegen_service;

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
    AutoTag {
        module_handle: String,
    },
    Ingest {
        module_handle: String,
    },
    Weave {
        module_handle: String,
    },
    LinkModule {
        module_handle: String,
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
            out_dir: _,
        } => {
            let target: TargetLanguage = lang.into();

            // 🎯 NOUVEAU : Déduction du domaine cible à partir du langage demandé
            let target_domain_str = match lang {
                CliTargetLanguage::Verilog | CliTargetLanguage::Vhdl => "hardware",
                _ => "software",
            };

            user_info!(
                "FORGE_GENERATE_INIT",
                json_value!({
                    "element_id": element_id,
                    "language": format!("{:?}", target),
                    "target_domain": target_domain_str,
                    "workspace_domain": ctx.active_domain
                })
            );

            // 🎯 On passe distinctement le domaine cible et le domaine de l'espace de travail
            match codegen_service::generate_source_code(
                &element_id,
                target_domain_str,
                &ctx.active_domain,
                &ctx.active_db,
                &ctx.storage,
            )
            .await
            {
                Ok(_) => user_success!("FORGE_SUCCESS", json_value!({"element": element_id})),
                Err(e) => raise_error!("ERR_FORGE_FAILED", error = e),
            }
        }

        CodeGenCommands::AutoTag { module_handle } => {
            // ⚠️ N'oublie pas de changer 'path' en 'module_handle' dans ton enum CodeGenCommands plus haut !
            user_info!(
                "CODE_AUTOTAG_START",
                json_value!({ "module": module_handle })
            );

            match codegen_service::auto_tag_module(
                &module_handle,
                &ctx.active_domain,
                &ctx.active_db,
                &ctx.storage,
            )
            .await
            {
                Ok(count) => {
                    if count > 0 {
                        user_success!(
                            "CODE_AUTOTAG_SUCCESS",
                            json_value!({ "module": module_handle, "tags_added": count })
                        );
                    } else {
                        user_info!(
                            "CODE_AUTOTAG_SKIPPED",
                            json_value!({ "module": module_handle, "hint": "Déjà synchronisé." })
                        );
                    }
                }
                Err(e) => raise_error!("ERR_AUTOTAG_FAILED", error = e),
            }
        }

        CodeGenCommands::Ingest { module_handle } => {
            user_info!(
                "CODE_INGEST_START",
                json_value!({ "module": module_handle })
            );

            match codegen_service::ingest_module(
                &module_handle,
                &ctx.active_domain,
                &ctx.active_db,
                &ctx.storage,
                ctx.is_test_mode,
            )
            .await
            {
                Ok(count) => user_success!(
                    "CODE_INGEST_SUCCESS",
                    json_value!({ "module": module_handle, "elements_ingested": count })
                ),
                Err(e) => raise_error!("ERR_INGEST_FAILED", error = e),
            }
        }

        CodeGenCommands::Weave { module_handle } => {
            user_info!("CODE_WEAVE_START", json_value!({ "module": module_handle }));

            match codegen_service::weave_module(
                &module_handle,
                &ctx.active_domain,
                &ctx.active_db,
                &ctx.storage,
                ctx.is_test_mode,
            )
            .await
            {
                Ok(final_path) => user_success!(
                    "CODE_WEAVE_SUCCESS",
                    json_value!({ "module": module_handle, "final_path": final_path })
                ),
                Err(e) => raise_error!("ERR_WEAVE_FAILED", error = e),
            }
        }

        CodeGenCommands::LinkModule { module_handle } => {
            user_info!("CODE_LINK_START", json_value!({ "module": module_handle }));

            match codegen_service::link_module(
                &module_handle,
                &ctx.active_domain,
                &ctx.active_db,
                &ctx.storage,
            )
            .await
            {
                Ok(count) => user_success!(
                    "CODE_LINK_SUCCESS",
                    json_value!({ "relations_resolved": count, "module": module_handle })
                ),
                Err(e) => raise_error!("ERR_CODE_LINK_FAILED", error = e),
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
    use raise_core::json_db::collections::manager::CollectionsManager;
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

        // 🎯 FIX : Ajout de la collection configs pour le ModelLoader
        let _ = sys_manager
            .create_collection("configs", &generic_schema)
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

        // 🎯 FIX : Mapping ontologique pour que le ModelLoader cherche dans la DB "mock_db"
        sys_manager
            .upsert_document(
                "configs",
                json_value!({
                    "_id": "ref:configs:handle:ontological_mapping",
                    "search_spaces": [ { "layer": "mock_db", "collection": "components" } ]
                }),
            )
            .await?;

        Ok(())
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_codegen_cli_dispatch() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage.clone());
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        // 🎯 FIX : On injecte directement via le moteur de stockage global
        inject_mock_codegen_config(&ctx.storage).await?;

        let _ = DbSandbox::mock_db(&manager).await;

        // 🎯 FIX : Le ModelLoader cherche les éléments sources dans la collection "components"
        manager
            .create_collection(
                "components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // 🎯 FIX : On simule un vrai composant MBSE, pas un morceau de code !
        let mock_component = json_value!({
            "_id": "sa:Processor_A",
            "handle": "Processor_A",
            "name": "Processor A",
            "type": "SystemComponent"
        });
        manager
            .upsert_document("components", mock_component)
            .await?;

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

        // 1. Injection du contexte système
        inject_mock_codegen_config(&ctx.storage).await?;
        let _ = DbSandbox::mock_db(&manager).await;

        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // 🎯 FIX : On déclare file_path et on crée le fichier physique en premier !
        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        let initial_code = "// @raise-handle: fn:test_fn\npub fn test_fn() { }";
        fs::write_sync(&file_path, initial_code)
            .map_err(|e| build_error!("ERR_TEST_FS", error = e))?;

        // 2. Création du nœud module Sémantique qui pointe vers file_path
        manager
            .create_collection(
                "modules",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        manager
            .insert_raw(
                "modules",
                &json_value!({
                    "_id": "ref:modules:handle:mod_test_weave",
                    "handle": "mod_test_weave",
                    "path": file_path.to_string_lossy().to_string()
                }),
            )
            .await?;

        // 3. Test de l'Ingestion (via le handle du module)
        let args_ingest = CodeGenArgs {
            command: CodeGenCommands::Ingest {
                module_handle: "mod_test_weave".to_string(),
            },
        };
        handle(args_ingest, ctx.clone()).await?;

        // 4. Vérification en base de données et simulation d'une modification par l'IA
        let query = raise_core::json_db::query::Query::new("code_elements");
        let db_result = raise_core::json_db::query::QueryEngine::new(&manager)
            .execute_query(query)
            .await?;

        if db_result.documents.is_empty() {
            raise_error!(
                "ERR_TEST_EMPTY_DB",
                error = "L'ingestion n'a créé aucun document."
            );
        }

        let mut doc = db_result.documents[0].clone();
        doc["body"] = json_value!("{ println!(\"RAISE_FORGE_OK\"); }");
        manager.upsert_document("code_elements", doc).await?;

        // 5. Test du Tissage (Weave)
        let args_weave = CodeGenArgs {
            command: CodeGenCommands::Weave {
                module_handle: "mod_test_weave".to_string(),
            },
        };
        handle(args_weave, ctx.clone()).await?;

        // 6. Validation finale du fichier physique
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
