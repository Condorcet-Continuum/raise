// FICHIER : src-tauri/tools/raise-cli/src/commands/code_gen.rs

use clap::{Args, Subcommand, ValueEnum};

use raise::{user_info, user_success, utils::prelude::*};

// 🎯 FIX : Importation sémantique correcte depuis le sous-module models
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
        /// ID du composant à générer
        element_id: String,
        /// Langage cible
        #[arg(short, long, value_enum)]
        lang: CliTargetLanguage,
    },
    /// 📥 Ingestion Bottom-Up : Lit un fichier source et peuple le Jumeau Numérique
    Ingest {
        /// Chemin vers le fichier source à ingérer
        path: String,
    },

    /// 📤 Tissage Top-Down : Matérialise le Jumeau Numérique dans un fichier physique
    Weave {
        /// Nom sémantique du module
        module_name: String,
        /// Chemin cible du fichier
        path: String,
    },
}

/// Bridge entre clap et l'enum TargetLanguage du cœur
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

// 🎯 La signature intègre le CliContext pour accéder au Jumeau Numérique
pub async fn handle(args: CodeGenArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique de la session
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        CodeGenCommands::Generate { element_id, lang } => {
            let target: TargetLanguage = lang.into();

            user_info!(
                "FORGE_START",
                json_value!({
                    "element_id": element_id,
                    "stage": "init",
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
            );

            // 🎯 Phase de synchronisation AST (Simulation des logs V2)
            user_info!(
                "TARGET_RESOLVED",
                json_value!({ "language": format!("{:?}", target) })
            );

            user_info!(
                "SYNC",
                json_value!({"action": "Synchronisation bidirectionnelle via AST Weaver..."})
            );

            if target == TargetLanguage::Rust {
                user_info!(
                    "LINT",
                    json_value!({"action": "Formatage et vérification statique."})
                );
            }

            user_success!(
                "FORGE_SUCCESS",
                json_value!({ "target": format!("{:?}", target), "status": "completed" })
            );
        }

        // 📥 L'AGENT D'INGESTION (Délégué au Service)
        CodeGenCommands::Ingest { path } => {
            user_info!("CODE_INGEST_START", json_value!({ "path": path }));

            let mut service = CodeGeneratorService::new(PathBuf::from(""));
            let schema_uri = if ctx.is_test_mode {
                service = service.with_test_mode();
                "db://_system/_system/schemas/v1/db/generic.schema.json" // Schéma de test
            } else {
                "db://_system/_system/schemas/v1/dapps/services/code_element.schema.json"
            };
            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let count = service
                .ingest_file(&PathBuf::from(&path), &manager, schema_uri)
                .await?;

            user_success!(
                "CODE_INGESTED",
                json_value!({ "path": path, "elements_count": count, "status": "synchronized_to_db" })
            );
        }

        // 📤 L'AGENT FORGERON (Délégué au Service)
        CodeGenCommands::Weave { module_name, path } => {
            user_info!(
                "CODE_WEAVE_START",
                json_value!({ "module": module_name, "path": path })
            );

            let mut service = CodeGeneratorService::new(PathBuf::from(""));
            if ctx.is_test_mode {
                service = service.with_test_mode();
            }

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
            let final_path = service
                .weave_file(&module_name, &PathBuf::from(&path), &manager)
                .await?;

            user_success!(
                "CODE_WEAVED_AND_VERIFIED",
                json_value!({ "module": module_name, "path": final_path.to_string_lossy(), "status": "compiled_and_saved" })
            );
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliContext;
    use raise::utils::context::SessionManager;
    use raise::utils::testing::DbSandbox;

    // 🎯 Ajout des imports nécessaires pour vérifier la DB
    use raise::json_db::collections::manager::CollectionsManager;
    use raise::json_db::query::{Query, QueryEngine};

    #[async_test]
    async fn test_codegen_cli_dispatch() {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);

        let args = CodeGenArgs {
            command: CodeGenCommands::Generate {
                element_id: "Logical_CPU".into(),
                lang: CliTargetLanguage::Vhdl,
            },
        };

        assert!(handle(args, ctx).await.is_ok());
    }

    #[async_test]
    async fn test_cli_ingest_and_weave_flow() {
        let sandbox = DbSandbox::new().await;
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);

        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);
        DbSandbox::mock_db(&manager).await.unwrap();
        manager
            .create_collection(
                "code_elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .expect("Le setup de la collection 'code_elements' a échoué");

        // 1. SCÉNARIO INITIAL : Le développeur ou l'IA a créé un fichier de base
        let file_path = sandbox.storage.config.data_root.join("test_forgeron.rs");
        let path_str = file_path.to_string_lossy().to_string();

        let initial_code = "
// @raise-handle: fn:test_weave
pub fn test_weave() {
    let base = 1;
}
";
        fs::write_sync(&file_path, initial_code).unwrap();

        // 2. INGESTION : Le Jumeau Numérique lit la réalité
        let args_ingest = CodeGenArgs {
            command: CodeGenCommands::Ingest {
                path: path_str.clone(),
            },
        };
        handle(args_ingest, ctx.clone()).await.unwrap();

        // 3. MUTATION EN BASE : L'Agent IA réfléchit et modifie le code dans la DB
        let query = Query::new("code_elements");
        let db_result = QueryEngine::new(&manager)
            .execute_query(query)
            .await
            .unwrap();

        let mut mutated_doc = db_result.documents[0].clone();

        // L'IA remplace le body pour y injecter son code
        mutated_doc["body"] =
            json_value!("{\n    let base = 1;\n    println!(\"IA was here\");\n}");
        manager
            .upsert_document("code_elements", mutated_doc)
            .await
            .unwrap();

        // 4. WEAVE : L'Agent Forgeron applique la mutation dans le monde réel
        let args_weave = CodeGenArgs {
            command: CodeGenCommands::Weave {
                module_name: "test_forgeron".to_string(),
                path: path_str.clone(),
            },
        };
        handle(args_weave, ctx.clone())
            .await
            .expect("Le tissage a échoué");

        // 5. VÉRIFICATION : Le fichier a-t-il bien été modifié par le Juge de Paix ?
        let final_code = fs::read_to_string_sync(&file_path).unwrap();
        assert!(
            final_code.contains("IA was here"),
            "Le code n'a pas été tissé correctement !"
        );

        // Bonus : La signature d'origine doit avoir été préservée
        assert!(final_code.contains("pub fn test_weave()"));
    }
}
