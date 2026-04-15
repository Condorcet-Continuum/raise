// FICHIER : src-tauri/tools/raise-cli/src/commands/rules.rs

use clap::{Args, Subcommand};
use raise::json_db::collections::manager::CollectionsManager;
use raise::utils::prelude::*;
use raise::workflow_engine::Mandate; // Import depuis le cœur
use raise::{raise_error, user_error, user_info, user_success, user_warn};

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
                json_value!({
                    "mandate_id": mandate_id,
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
            );

            // 1. Fetch via la logique métier du Core
            let mandate = Mandate::fetch_from_store(&manager, &mandate_id).await?;

            // 2. Analyse via la logique métier du Core
            let analyses = mandate.analyze_vetos();

            if analyses.is_empty() {
                user_warn!(
                    "RULES_ANALYZE_EMPTY",
                    json_value!({ "hint": "Aucune règle AST trouvée dans ce Mandat." })
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
                            // 🎯 FIX : On force le .to_string() pour éviter un crash de sérialisation JSON
                            json_value!({"rule": analysis.rule_name, "error": err.to_string()})
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

        RulesCommands::Evaluate { collection, handle } => {
            user_info!(
                "RULES_EVAL_START",
                json_value!({"handle": handle, "col": collection})
            );

            // 1. On récupère le document via l'API Core
            let doc = match manager.get_document(&collection, &handle).await? {
                Some(d) => d,
                None => raise_error!(
                    "ERR_DOC_NOT_FOUND",
                    error = format!("Document '{}' introuvable", handle)
                ),
            };

            // 2. On extrait son ID de manière sécurisée
            let doc_id = match doc.get("_id").and_then(|v| v.as_str()) {
                Some(id) => id.to_string(),
                None => raise_error!(
                    "ERR_DB_CORRUPTION",
                    error = format!("Le document '{}' ne possède pas d'_id.", handle)
                ),
            };

            user_info!(
                "RULES_COMPUTING",
                json_value!({"action": "Délégation au moteur JSON DB..."})
            );

            // 3. 🚀 L'ÉLÉGANCE ARCHITECTURALE :
            // On fait un "update" avec un patch vide JSON {}.
            // Le CollectionsManager va charger le document, le faire passer dans `prepare_document()`,
            // ce qui déclenche NATIVEMENT `apply_business_rules` et l'évaluation de l'AST !
            let updated_doc = manager
                .update_document(&collection, &doc_id, json_value!({}))
                .await?;

            user_success!(
                "RULES_EVAL_SUCCESS",
                json_value!({
                    "target": handle,
                    "status": "computed_and_saved",
                    "new_state": updated_doc["ui"] // On affiche la partie UI pour prouver la mutation
                })
            );
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES (Parsing CLI - "Zéro Dette")
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
    fn test_parse_analyze_command() -> RaiseResult<()> {
        let args = vec!["test", "analyze", "mandate_v1"];

        let cli = match TestCli::try_parse_from(args) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_TEST_PARSE_FAILED", error = e.to_string()),
        };

        match cli.args.command {
            RulesCommands::Analyze { mandate_id } => {
                assert_eq!(mandate_id, "mandate_v1");
                Ok(())
            }
            _ => raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Mauvaise commande parsée"
            ),
        }
    }

    #[test]
    fn test_parse_evaluate_command() -> RaiseResult<()> {
        let args = vec![
            "test",
            "evaluate",
            "--collection",
            "dapps",
            "--handle",
            "defi",
        ];

        let cli = match TestCli::try_parse_from(args) {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_TEST_PARSE_FAILED", error = e.to_string()),
        };

        match cli.args.command {
            RulesCommands::Evaluate { collection, handle } => {
                assert_eq!(collection, "dapps");
                assert_eq!(handle, "defi");
                Ok(())
            }
            _ => raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Mauvaise commande parsée"
            ),
        }
    }
}
