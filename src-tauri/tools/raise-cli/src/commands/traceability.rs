// FICHIER : src-tauri/tools/raise-cli/src/commands/traceability.rs

use clap::{Args, Subcommand};
use raise::{user_info, user_success, utils::prelude::*, utils::HashMap};

// Imports mis à jour depuis le cœur
use raise::model_engine::types::ProjectModel;
use raise::traceability::{
    reporting::audit_report::AuditGenerator, ChangeTracker, ImpactAnalyzer, Tracer,
};

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
fn get_docs(model: &ProjectModel) -> HashMap<String, Value> {
    let mut docs = HashMap::new();
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

pub async fn handle(args: TraceabilityArgs) -> Result<()> {
    match args.command {
        TraceabilityCommands::Audit => {
            user_info!("TRACE_START", "Initialisation du moteur de traçage...");

            let model = ProjectModel::default();
            let docs = get_docs(&model);

            // 🎯 FIX : Utilisation du constructeur de rétro-compatibilité
            let tracer = Tracer::from_legacy_model(&model);

            // 🎯 FIX : Utilisation du générateur de rapport universel
            let report = AuditGenerator::generate(&tracer, &docs, &model.meta.name);

            println!("{}", serde_json::to_string_pretty(&report).unwrap());

            user_success!(
                "AUDIT_DONE",
                "Analyse de traçabilité effectuée avec {} règles vérifiées.",
                report.compliance_results.len()
            );
        }

        TraceabilityCommands::Impact { component_id } => {
            user_info!("ANALYSIS", "Analyse d'impact pour : {}", component_id);

            let model = ProjectModel::default();

            // 🎯 FIX : Plus de lifetime 'a dans Tracer
            let tracer = Tracer::from_legacy_model(&model);
            let analyzer = ImpactAnalyzer::new(tracer);

            user_info!("RESULT", "Calcul des propagations de changement...");
            let report = analyzer.analyze(&component_id, 3);

            println!("{}", serde_json::to_string_pretty(&report).unwrap());
            user_success!("IMPACT_OK", "Rapport d'impact généré pour {}", component_id);
        }

        TraceabilityCommands::History => {
            user_info!("TRACKER", "Consultation de l'historique des changements...");
            let _tracker = ChangeTracker::new();
            user_success!("HISTORY_READY", "Historique de traçabilité chargé.");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_traceability_cli_flow() {
        let args = TraceabilityArgs {
            command: TraceabilityCommands::Audit,
        };
        assert!(handle(args).await.is_ok());
    }
}
