// FICHIER : src-tauri/tools/raise-cli/src/commands/genetics.rs

use clap::{Args, Subcommand};
use raise::{user_info, user_success, user_warn, utils::prelude::*}; // 🎯 Façade Unique RAISE

// Imports du Core (Logique métier)
use raise::genetics::engine::GeneticConfig;

// 🎯 Import du contexte global CLI
use crate::CliContext;

/// Commandes pour le Moteur Génétique (Raise Genetics Engine)
#[derive(Args, Clone, Debug)]
pub struct GeneticsArgs {
    #[command(subcommand)]
    pub command: GeneticsCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum GeneticsCommands {
    /// Lance une simulation d'évolution sur une population
    Evolve {
        /// Taille de la population initiale
        #[arg(short, long, default_value = "100")]
        population: usize,

        /// Nombre de générations à simuler
        #[arg(short, long, default_value = "50")]
        generations: usize,

        /// Taux de mutation (0.0 - 1.0)
        #[arg(short, long, default_value = "0.05")]
        mutation_rate: f32,

        /// Taux de croisement (crossover)
        #[arg(short, long, default_value = "0.8")]
        crossover_rate: f32,
    },
    /// Inspecte le génome du meilleur individu
    Inspect {
        /// ID spécifique d'un individu ou front de Pareto
        #[arg(short, long)]
        id: Option<String>,
    },
}

pub async fn handle(args: GeneticsArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        GeneticsCommands::Evolve {
            population,
            generations,
            mutation_rate,
            crossover_rate,
        } => {
            user_info!(
                "GENETICS_INIT",
                json_value!({
                    "active_domain": ctx.active_domain,
                    "active_user": ctx.active_user
                })
            );

            // 1. Création de la configuration réelle
            let config = GeneticConfig {
                population_size: population,
                max_generations: generations,
                mutation_rate,
                crossover_rate,
                elitism_count: 5,
            };

            // 2. Validation des hyperparamètres
            if !(0.0..=1.0).contains(&config.mutation_rate) {
                user_warn!(
                    "GENETICS_CONFIG_BOUNDS",
                    json_value!({
                        "field": "mutation_rate",
                        "value": config.mutation_rate,
                        "hint": "Le taux devrait être entre 0.0 et 1.0."
                    })
                );
            }

            user_info!(
                "GENETICS_READY",
                json_value!({
                    "pop_size": config.population_size,
                    "max_gen": config.max_generations
                })
            );

            // TODO: Intégration future avec le GeneticEngine et le SystemModelProvider

            user_success!(
                "GENETICS_SUCCESS",
                json_value!({"status": "Simulation configurée et prête pour le modèle système."})
            );
        }
        GeneticsCommands::Inspect { id } => {
            let target = id.as_deref().unwrap_or("Pareto Front Best");
            user_info!("GENETICS_INSPECT", json_value!({ "target": target }));
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES (Conformité "Zéro Dette")
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::testing::DbSandbox;

    #[async_test]
    #[serial_test::serial] // 🎯 FIX : Empêche les collisions de sandbox
    async fn test_genetics_config_mapping() -> RaiseResult<()> {
        raise::json_db::jsonld::VocabularyRegistry::init_mock_for_tests();

        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = crate::context::SessionManager::new(storage.clone());

        let ctx = crate::CliContext::mock(AppConfig::get(), session_mgr, storage);

        let args = GeneticsArgs {
            command: GeneticsCommands::Evolve {
                population: 10,
                generations: 5,
                mutation_rate: 0.1,
                crossover_rate: 0.9,
            },
        };

        handle(args, ctx).await
    }
}
