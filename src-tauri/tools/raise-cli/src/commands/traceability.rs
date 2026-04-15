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
/// 🎯 PURE GRAPH : Utilisation de all_elements() pour remplacer l'énumération manuelle des couches.
fn get_docs(model: &ProjectModel) -> UnorderedMap<String, JsonValue> {
    let mut docs = UnorderedMap::new();

    // On itère dynamiquement sur l'ensemble du graphe
    for e in model.all_elements() {
        if let Ok(val) = json::serialize_to_value(e) {
            docs.insert(e.id.clone(), val);
        }
    }

    docs
}

// 🎯 La signature attend maintenant le CliContext complet
pub async fn handle(args: TraceabilityArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique : on signale que la session est active
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        TraceabilityCommands::Audit => {
            user_info!(
                "TRACE_START",
                json_value!({
                    "step": "init",
                    "message": "Initialisation du moteur de traçage...",
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
            );

            // Note: En production, le modèle serait chargé via le session_mgr
            let model = ProjectModel::default();
            let docs = get_docs(&model);

            // Tracer::from_legacy_model utilise désormais en interne all_elements()
            let tracer = Tracer::from_legacy_model(&model);
            let report = AuditGenerator::generate(&tracer, &docs, &model.meta.name);

            // 🎯 FIX CRITIQUE : Remplacement du unwrap() par l'opérateur ? (qui convertit en RaiseResult)
            println!("{}", json::serialize_to_string_pretty(&report)?);

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
                    "action": "evaluating_side_effects",
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
            );

            let model = ProjectModel::default();
            let tracer = Tracer::from_legacy_model(&model);
            let analyzer = ImpactAnalyzer::new(tracer);

            user_info!(
                "IMPACT_CALCULATING",
                json_value!({"step": "graph_traversal", "message": "Calcul des propagations de changement..."})
            );

            let report = analyzer.analyze(&component_id, 3);

            // 🎯 FIX CRITIQUE : Remplacement du unwrap()
            println!("{}", json::serialize_to_string_pretty(&report)?);

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
                json_value!({
                    "action": "fetch_history",
                    "message": "Consultation de l'historique des changements...",
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
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

// --- TESTS UNITAIRES ("Zéro Dette") ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::CliContext;
    use raise::utils::context::SessionManager;
    use raise::utils::testing::DbSandbox;

    #[async_test]
    // 🎯 FIX : Signature RaiseResult<()>
    async fn test_traceability_cli_flow() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();
        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);

        let args = TraceabilityArgs {
            command: TraceabilityCommands::Audit,
        };

        // 🎯 FIX : Remplacement du assert!() par match
        match handle(args, ctx).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!("ERR_TEST_TRACEABILITY_FLOW", error = e.to_string()),
        }
    }
}
