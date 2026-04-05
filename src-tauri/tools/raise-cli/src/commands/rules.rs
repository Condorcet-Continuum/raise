// FICHIER : src-tauri/tools/raise-cli/src/commands/rules.rs

use clap::{Args, Subcommand};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*;
use raise::workflow_engine::Mandate; // Import depuis le cœur
use raise::{user_error, user_info, user_success, user_warn};

use crate::CliContext;

#[derive(Args, Clone, Debug)]
pub struct RulesArgs {
    #[command(subcommand)]
    pub command: RulesCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum RulesCommands {
    /// Analyse statique d'une règle AST ou des règles d'un Mandat
    Analyze {
        /// L'ID ou le handle du mandat contenant les Lignes Rouges
        mandate_id: String,
    },
}

pub async fn handle(args: RulesArgs, ctx: CliContext) -> RaiseResult<()> {
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        RulesCommands::Analyze { mandate_id } => {
            user_info!(
                "RULES_ANALYZE_START",
                json_value!({
                    "mandate_id": mandate_id,
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
            );

            let manager = CollectionsManager::new(&ctx.storage, &ctx.active_domain, &ctx.active_db);

            // 1. Fetch via la logique métier du Core
            let mandate = Mandate::fetch_from_store(&manager, &mandate_id).await?;

            // 2. Analyse via la logique métier du Core
            let analyses = mandate.analyze_vetos();

            if analyses.is_empty() {
                user_warn!(
                    "RULES_ANALYZE_EMPTY",
                    json_value!({ "hint": "Aucune règle AST trouvée." })
                );
                return Ok(());
            }

            println!("\n🔍 --- RÉSULTATS DE L'ANALYSE STATIQUE ---");
            let mut error_count = 0;

            // 3. Affichage
            for analysis in &analyses {
                println!("🛡️  Règle : {}", analysis.rule_name);
                match &analysis.status {
                    Ok(deps) => {
                        println!("   ✅ Syntaxe & Profondeur : Conformes");
                        println!("   🔗 Dépendances : {:?}", deps);
                    }
                    Err(err) => {
                        println!("   ❌ ÉCHEC : {}", err);
                        user_error!(
                            "RULES_AST_INVALID",
                            json_value!({"rule": analysis.rule_name, "error": err})
                        );
                        error_count += 1;
                    }
                }
                println!();
            }

            if error_count == 0 {
                user_success!(
                    "RULES_ANALYZE_SUCCESS",
                    json_value!({"status": "all_rules_valid", "count": analyses.len()})
                );
            } else {
                user_error!("RULES_ANALYZE_FAILED", json_value!({"errors": error_count}));
            }
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES (Parsing CLI)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(flatten)]
        args: RulesArgs,
    }

    #[test]
    fn verify_cli_structure() {
        TestCli::command().debug_assert();
    }

    #[test]
    fn test_parse_analyze_command() {
        let args = vec!["test", "analyze", "mandate_v1"];
        let cli = TestCli::try_parse_from(args).unwrap();

        match cli.args.command {
            RulesCommands::Analyze { mandate_id } => {
                assert_eq!(mandate_id, "mandate_v1");
            }
        }
    }
}
