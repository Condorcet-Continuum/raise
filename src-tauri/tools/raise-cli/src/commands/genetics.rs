// FICHIER : src-tauri/tools/raise-cli/src/commands/genetics.rs

use clap::{Args, Subcommand};
use raise::{user_info, user_success, user_warn, utils::prelude::*}; // 🎯 Ajout de user_warn

// Imports du Core (chemin relatif à l'arborescence src-tauri)
use raise::genetics::engine::GeneticConfig;

// 🎯 NOUVEAU : Import du contexte global CLI
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
        population: usize, // Changé en usize pour correspondre à GeneticConfig

        /// Nombre de générations à simuler
        #[arg(short, long, default_value = "50")]
        generations: usize, // Changé en usize pour correspondre à GeneticConfig

        /// Taux de mutation (0.0 - 1.0)
        #[arg(short, long, default_value = "0.05")]
        mutation_rate: f32,

        /// Taux de croisement (crossover)
        #[arg(short, long, default_value = "0.8")]
        crossover_rate: f32,
    },
    /// Inspecte le génome du meilleur individu
    Inspect {
        #[arg(short, long)]
        id: Option<String>,
    },
}

// 🎯 La signature intègre le CliContext
pub async fn handle(args: GeneticsArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        GeneticsCommands::Evolve {
            population,
            generations,
            mutation_rate,
            crossover_rate,
        } => {
            // 🎯 Mise en conformité stricte JSON
            user_info!(
                "GENETICS_START",
                json!({"action": "Initialisation du moteur NSGA-II..."})
            );

            // 1. Création de la configuration réelle du Core
            let config = GeneticConfig {
                population_size: population,
                max_generations: generations,
                mutation_rate,
                crossover_rate,
                elitism_count: 5, // Valeur par défaut
            };

            // Validation conforme Clippy
            if !(0.0..=1.0).contains(&config.mutation_rate) {
                // 🎯 Utilisation de user_warn avec payload structuré
                user_warn!(
                    "CONFIG_WARN",
                    json!({
                        "issue": "Mutation rate hors bornes, ajustement requis.",
                        "field": "mutation_rate",
                        "value": config.mutation_rate
                    })
                );
            }

            user_info!(
                "CONFIG_READY",
                json!({
                    "population": config.population_size,
                    "generations": config.max_generations,
                    "mutation": config.mutation_rate,
                    "crossover": config.crossover_rate,
                    "action": "initialize_genetic_engine"
                })
            );

            // TODO: Ici nous instancierons le GeneticEngine avec SystemModelProvider
            // Pour l'instant on valide que la structure de config est acceptée

            // 🎯 Payload JSON pour le succès
            user_success!(
                "GENETICS_DONE",
                json!({"status": "Simulation prête à être exécutée sur le modèle système."})
            );
        }
        GeneticsCommands::Inspect { id } => {
            let target = id.as_deref().unwrap_or("Meilleur Pareto Front");
            user_info!(
                "INSPECT_TARGET",
                json!({ "target": target }) // format!("{:?}", target) n'est plus nécessaire
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
    use raise::utils::mock::DbSandbox;
    use raise::utils::session::SessionManager;
    use raise::utils::Arc;

    #[tokio::test]
    async fn test_genetics_config_mapping() {
        // 🎯 On simule le contexte global pour le test
        let sandbox = DbSandbox::new().await;
        let storage = Arc::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr,
            storage,
        };

        let args = GeneticsArgs {
            command: GeneticsCommands::Evolve {
                population: 10,
                generations: 5,
                mutation_rate: 0.1,
                crossover_rate: 0.9,
            },
        };

        assert!(handle(args, ctx).await.is_ok());
    }
}
