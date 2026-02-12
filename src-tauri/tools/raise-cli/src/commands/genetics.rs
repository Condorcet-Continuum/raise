use clap::{Args, Subcommand};
use raise::{user_info, user_success, utils::prelude::*};

// Imports du Core (chemin relatif à l'arborescence src-tauri)
use raise::genetics::engine::GeneticConfig;

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

pub async fn handle(args: GeneticsArgs) -> Result<()> {
    match args.command {
        GeneticsCommands::Evolve {
            population,
            generations,
            mutation_rate,
            crossover_rate,
        } => {
            user_info!("GENETICS_START", "Initialisation du moteur NSGA-II...");

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
                user_info!(
                    "CONFIG_WARN",
                    "Mutation rate hors bornes, ajustement requis."
                );
            }

            user_info!(
                "CONFIG_READY",
                "Pop: {} | Gen: {} | Mut: {} | Cross: {}",
                config.population_size,
                config.max_generations,
                config.mutation_rate,
                config.crossover_rate
            );

            // TODO: Ici nous instancierons le GeneticEngine avec SystemModelProvider
            // Pour l'instant on valide que la structure de config est acceptée

            user_success!(
                "GENETICS_DONE",
                "Simulation prête à être exécutée sur le modèle système."
            );
        }
        GeneticsCommands::Inspect { id } => {
            let target = id.as_deref().unwrap_or("Meilleur Pareto Front");
            user_info!("INSPECT", "Analyse de : {}", target);
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_genetics_config_mapping() {
        let args = GeneticsArgs {
            command: GeneticsCommands::Evolve {
                population: 10,
                generations: 5,
                mutation_rate: 0.1,
                crossover_rate: 0.9,
            },
        };
        assert!(handle(args).await.is_ok());
    }
}
