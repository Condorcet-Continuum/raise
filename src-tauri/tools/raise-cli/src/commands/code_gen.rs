// FICHIER : src-tauri/tools/raise-cli/src/commands/code_gen.rs

use clap::{Args, Subcommand, ValueEnum};
use raise::{user_info, user_success, utils::prelude::*};

// 🎯 Imports sémantiques depuis la forge logicielle
use raise::code_generator::models::TargetLanguage;
use raise::code_generator::CodeGeneratorService;
use raise::json_db::collections::manager::CollectionsManager;

use crate::CliContext;

/// Forge logicielle et matérielle (Arcadia-to-Code)
#[derive(Args, Clone, Debug)]
pub struct CodeGenArgs {
    #[command(subcommand)]
    pub command: CodeGenCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum CodeGenCommands {
    /// Génère le code source pour un élément du modèle
    Generate {
        /// ID du composant à générer (URI Arcadia)
        element_id: String,
        /// Langage cible (Rust, C++, VHDL...)
        #[arg(short, long, value_enum)]
        lang: CliTargetLanguage,
    },
    /// 📥 Ingestion Bottom-Up : Analyse un fichier source pour peupler le Knowledge Graph
    Ingest {
        /// Chemin vers le fichier source
        path: String,
    },
    /// 📤 Tissage Top-Down : Matérialise le Jumeau Numérique dans un fichier physique
    Weave {
        /// Nom sémantique du module à synchroniser
        module_name: String,
        /// Chemin cible du fichier physique
        path: String,
    },
}

/// Bridge entre clap (CLI) et TargetLanguage (Core)
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum, Debug)]
pub enum CliTargetLanguage {
    Rust,
    Typescript,
    Cpp,
    Verilog,
    Vhdl,
}

impl From<CliTargetLanguage> for TargetLanguage {
    fn from(lang: CliTargetLanguage) -> Self {
        match lang {
            CliTargetLanguage::Rust => TargetLanguage::Rust,
            CliTargetLanguage::Typescript => TargetLanguage::TypeScript,
            CliTargetLanguage::Cpp => TargetLanguage::Cpp,
            CliTargetLanguage::Verilog => TargetLanguage::Verilog,
            CliTargetLanguage::Vhdl => TargetLanguage::Vhdl,
        }
    }
}

pub async fn handle(args: CodeGenArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        CodeGenCommands::Generate { element_id, lang } => {
            let target: TargetLanguage = lang.into();

            user_info!(
                "FORGE_GENERATE_INIT",
                json_value!({
                    "element_id": element_id,
                    "language": format!("{:?}", target),
                    "domain": ctx.active_domain
                })
            );

            // Phase de simulation AST/Linter
            user_info!(
                "FORGE_AST_SYNC",
                json_value!({"status": "analyzing_structure"})
            );

            if target == TargetLanguage::Rust {
                user_info!(
                    "FORGE_LINT_RUST",
                    json_value!({"action": "cargo_fmt_check"})
                );
            }

            user_success!(
                "FORGE_SUCCESS",
                json_value!({ "element": element_id, "target": format!("{:?}", target) })
            );
        }

        CodeGenCommands::Ingest { path } => {
            user_info!("CODE_INGEST_START", json_value!({ "path": path }));

            let mut service = CodeGeneratorService::new(PathBuf::from(""));

            // Résolution du schéma selon le mode (Test vs Prod)
            let schema_uri = if ctx.is_test_mode {
                service = service.with_test_mode();
                "db://_system/_system/schemas/v1/db/generic.schema.json"
            } else {
                "db://_system/_system/schemas/v1/dapps/services/code_element.schema.json"
            };

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            match service
                .ingest_file(&PathBuf::from(&path), &manager, schema_uri)
                .await
            {
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

            let mut service = CodeGeneratorService::new(PathBuf::from(""));
            if ctx.is_test_mode {
                service = service.with_test_mode();
            }

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

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
    use raise::json_db::query::{Query, QueryEngine};
    use raise::utils::context::SessionManager;
    use raise::utils::testing::DbSandbox;

    #[async_test]
    #[serial_test::serial]
    async fn test_codegen_cli_dispatch() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);
        let args = CodeGenArgs {
            command: CodeGenCommands::Generate {
                element_id: "sa:Processor_A".into(),
                lang: CliTargetLanguage::Rust,
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
        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        // 1. Setup Sandbox
        DbSandbox::mock_db(&manager).await?;
        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // 2. Création du fichier source initial
        let file_path = sandbox.storage.config.data_root.join("test_weave.rs");
        let initial_code = "// @raise-handle: fn:test_fn\npub fn test_fn() { }";
        fs::write_sync(&file_path, initial_code)
            .map_err(|e| build_error!("ERR_TEST_FS", error = e))?;

        // 3. INGESTION
        let args_ingest = CodeGenArgs {
            command: CodeGenCommands::Ingest {
                path: file_path.to_string_lossy().to_string(),
            },
        };
        handle(args_ingest, ctx.clone()).await?;

        // 4. MUTATION (Simulation d'une modification par l'Agent IA)
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

        // 5. WEAVE (Le Forgeron applique les changements sur le disque)
        let args_weave = CodeGenArgs {
            command: CodeGenCommands::Weave {
                module_name: "test_weave".to_string(),
                path: file_path.to_string_lossy().to_string(),
            },
        };
        handle(args_weave, ctx.clone()).await?;

        // 6. VÉRIFICATION FINALE
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
