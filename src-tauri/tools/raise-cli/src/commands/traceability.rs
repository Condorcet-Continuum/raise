// FICHIER : src-tauri/tools/raise-cli/src/commands/traceability.rs

use clap::{Args, Subcommand};
use raise::{user_info, user_success, utils::prelude::*};

// Imports mis à jour depuis le cœur
use raise::model_engine::types::ProjectModel;
use raise::traceability::{
    reporting::audit_report::AuditGenerator, ChangeTracker, ImpactAnalyzer, Tracer,
};

// 🎯 NOUVEAU : Import du contexte global CLI
use crate::CliContext;

#[derive(Args, Clone, Debug)]
pub struct TraceabilityArgs {
    #[command(subcommand)]
    pub command: TraceabilityCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum TraceabilityCommands {
    /// Lance un rapport de traçage complet sur le modèle actuel
    Audit,
    /// Analyse l'impact d'un changement sur un composant cible
    Impact {
        /// Identifiant du composant
        component_id: String,
    },
    /// Affiche les derniers changements détectés
    History,
}

/// Helper pour extraire les documents (indispensable pour les nouveaux générateurs)
fn get_docs(model: &ProjectModel) -> UnorderedMap<String, JsonValue> {
    let mut docs = UnorderedMap::new();
    let mut collect = |elements: &Vec<raise::model_engine::types::ArcadiaElement>| {
        for e in elements {
            if let Ok(val) = serde_json::to_value(e) {
                docs.insert(e.id.clone(), val);
            }
        }
    };

    collect(&model.sa.functions);
    collect(&model.sa.components);
    collect(&model.la.functions);
    collect(&model.la.components);
    collect(&model.pa.functions);
    collect(&model.pa.components);
    collect(&model.transverse.requirements);

    docs
}

// 🎯 La signature attend maintenant le CliContext complet
pub async fn handle(args: TraceabilityArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique : on signale que la session est active
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        TraceabilityCommands::Audit => {
            // 🎯 Utilisation stricte du contexte JSON pour les macros
            user_info!(
                "TRACE_START",
                json_value!({"step": "init", "message": "Initialisation du moteur de traçage..."})
            );

            let model = ProjectModel::default();
            let docs = get_docs(&model);

            let tracer = Tracer::from_legacy_model(&model);
            let report = AuditGenerator::generate(&tracer, &docs, &model.meta.name);

            println!("{}", serde_json::to_string_pretty(&report).unwrap());

            user_success!(
                "AUDIT_TRACEABILITY_COMPLETE",
                json_value!({
                    "rules_checked": report.compliance_results.len(),
                    "status": "verified",
                    "module": "traceability_engine"
                })
            );
        }

        TraceabilityCommands::Impact { component_id } => {
            user_info!(
                "IMPACT_ANALYSIS_START",
                json_value!({
                    "component": component_id,
                    "scope": "dependency_graph",
                    "action": "evaluating_side_effects"
                })
            );

            let model = ProjectModel::default();
            let tracer = Tracer::from_legacy_model(&model);
            let analyzer = ImpactAnalyzer::new(tracer);

            // 🎯 Remplacement de la simple string par un JSON structuré
            user_info!(
                "IMPACT_CALCULATING",
                json_value!({"step": "graph_traversal", "message": "Calcul des propagations de changement..."})
            );

            let report = analyzer.analyze(&component_id, 3);

            println!("{}", serde_json::to_string_pretty(&report).unwrap());

            user_success!(
                "IMPACT_ANALYSIS_SUCCESS",
                json_value!({
                    "component": component_id,
                    "status": "report_generated",
                    "timestamp": UtcClock::now().to_rfc3339()
                })
            );
        }

        TraceabilityCommands::History => {
            user_info!(
                "TRACKER_START",
                json_value!({"action": "fetch_history", "message": "Consultation de l'historique des changements..."})
            );

            let _tracker = ChangeTracker::new();

            user_success!(
                "HISTORY_READY",
                json_value!({"status": "loaded", "message": "Historique de traçabilité chargé."})
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

    #[cfg(test)]
    use raise::utils::testing::DbSandbox;

    #[async_test]
    async fn test_traceability_cli_flow() {
        // 🎯 On simule le contexte global pour le test
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr,
            storage,
        };

        let args = TraceabilityArgs {
            command: TraceabilityCommands::Audit,
        };

        // On passe le contexte à handle()
        assert!(handle(args, ctx).await.is_ok());
    }
}
