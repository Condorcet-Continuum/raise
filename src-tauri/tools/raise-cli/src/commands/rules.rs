// FICHIER : src-tauri/tools/raise-cli/src/commands/rules.rs

use clap::{Args, Subcommand};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*; // 🎯 Façade Unique RAISE
use raise::workflow_engine::Mandate;

use crate::CliContext;

#[derive(Args, Clone, Debug)]
pub struct RulesArgs {
    #[command(subcommand)]
    pub command: RulesCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum RulesCommands {
    /// Analyse statique des règles AST d'un Mandat (Vetos/Lignes Rouges)
    Analyze {
        /// ID ou handle du mandat à auditer
        mandate_id: String,
    },
    /// 🚀 Évalue et applique les règles (x_rules) d'un schéma sur un document
    Evaluate {
        #[arg(long)]
        collection: String,
        #[arg(long)]
        handle: String,
    },
}

pub async fn handle(args: RulesArgs, ctx: CliContext) -> RaiseResult<()> {
    let _ = ctx.session_mgr.touch().await;
    let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

    match args.command {
        RulesCommands::Analyze { mandate_id } => {
            user_info!(
                "RULES_ANALYZE_START",
                json_value!({ "mandate": mandate_id })
            );

            // 1. Récupération du Mandat (Logique Core)
            let mandate = Mandate::fetch_from_store(&manager, &mandate_id).await?;
            let analyses = mandate.analyze_vetos();

            if analyses.is_empty() {
                user_warn!(
                    "RULES_ANALYZE_EMPTY",
                    json_value!({ "hint": "Aucune règle trouvée." })
                );
                return Ok(());
            }

            println!("\n🔍 --- RÉSULTATS DE L'ANALYSE STATIQUE ---");
            let mut error_count = 0;

            for analysis in &analyses {
                println!("🛡️  Règle : {}", analysis.rule_name);
                match &analysis.status {
                    Ok(deps) => {
                        println!("   ✅ Syntaxe conforme");
                        println!("   🔗 Dépendances : {:?}", deps);
                    }
                    Err(err) => {
                        println!("   ❌ ÉCHEC : {}", err);
                        user_error!(
                            "RULES_AST_INVALID",
                            json_value!({"rule": analysis.rule_name, "error": err.to_string()})
                        );
                        error_count += 1;
                    }
                }
            }

            if error_count == 0 {
                user_success!(
                    "RULES_ANALYZE_SUCCESS",
                    json_value!({ "count": analyses.len() })
                );
                Ok(())
            } else {
                // 🎯 FIX : On retourne une erreur pour signaler l'échec au Shell (Exit Code != 0)
                raise_error!(
                    "ERR_RULES_VIOLATION",
                    error = format!("{} violation(s) détectée(s) dans le mandat.", error_count),
                    context = json_value!({ "mandate": mandate_id, "errors": error_count })
                )
            }
        }

        RulesCommands::Evaluate { collection, handle } => {
            user_info!("RULES_EVAL_START", json_value!({ "handle": handle }));

            let doc = manager
                .get_document(&collection, &handle)
                .await?
                .ok_or_else(|| {
                    build_error!(
                        "ERR_DOC_NOT_FOUND",
                        error = format!("Document '{}' introuvable", handle)
                    )
                })?;

            let doc_id = doc
                .get("_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| build_error!("ERR_DB_CORRUPTION", error = "Document sans _id"))?;

            // 🚀 L'ÉLÉGANCE RAISE : Un update vide {} force le passage dans `apply_business_rules`
            let updated_doc = manager
                .update_document(&collection, doc_id, json_value!({}))
                .await?;

            user_success!(
                "RULES_EVAL_SUCCESS",
                json_value!({
                    "target": handle,
                    "status": "computed",
                    "ui_state": updated_doc["ui"]
                })
            );
            Ok(())
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Conformité "Zéro Dette")
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::testing::DbSandbox;

    #[test]
    #[serial_test::serial]
    fn test_parse_analyze_logic() -> RaiseResult<()> {
        use clap::Parser;
        #[derive(Parser)]
        struct TestCli {
            #[command(flatten)]
            args: RulesArgs,
        }

        let cli = TestCli::try_parse_from(vec!["test", "analyze", "m_v1"])
            .map_err(|e| build_error!("ERR_TEST", error = e))?;

        if let RulesCommands::Analyze { mandate_id } = cli.args.command {
            assert_eq!(mandate_id, "m_v1");
            Ok(())
        } else {
            raise_error!("ERR_TEST_FAIL", error = "Parsing failed")
        }
    }

    /// 🎯 TEST D'INTÉGRATION : Vérification du moteur de règles
    #[async_test]
    #[serial_test::serial]
    async fn test_rules_evaluation_workflow() -> RaiseResult<()> {
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = crate::context::SessionManager::new(storage.clone());
        let ctx = crate::CliContext::mock(AppConfig::get(), session_mgr, storage);
        let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

        DbSandbox::mock_db(&manager).await?;
        manager
            .create_collection(
                "items",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        // Insertion d'un document test
        manager
            .upsert_document("items", json_value!({ "_id": "t1", "handle": "test_item" }))
            .await?;

        let args = RulesArgs {
            command: RulesCommands::Evaluate {
                collection: "items".into(),
                handle: "test_item".into(),
            },
        };
        handle(args, ctx).await
    }
}
