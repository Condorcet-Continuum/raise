use clap::{Args, Subcommand};

use raise::{user_info, user_success, utils::prelude::*};

// Import de la fonction principale de topologie
use raise::spatial_engine::get_spatial_topology;

/// Pilotage du Spatial Engine (Visualisation 3D & Jumeau Numérique)
#[derive(Args, Clone, Debug)]
pub struct SpatialArgs {
    #[command(subcommand)]
    pub command: SpatialCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum SpatialCommands {
    /// Génère la topologie 3D actuelle et affiche les métadonnées
    Topology,
    /// Liste les composants présentant une instabilité (vibration)
    Health,
}

pub async fn handle(args: SpatialArgs) -> RaiseResult<()> {
    match args.command {
        SpatialCommands::Topology => {
            user_info!(
                "SPATIAL",
                "Génération procédurale de la topologie Arcadia..."
            );

            // Récupération du graphe spatial
            let graph = get_spatial_topology();

            user_info!(
                "GRAPH_STATS_NODES",
                json!({
                    "count": graph.meta.node_count,
                    "complexity": if graph.meta.node_count > 1000 { "high" } else { "standard" }
                })
            );
            // Accès aux statistiques par couche (OA, SA, LA, PA, Chaos)
            user_info!(
                "GRAPH_LAYERS_DISTRIBUTION",
                json!({
                    "oa": graph.meta.layer_distribution[0],
                    "sa": graph.meta.layer_distribution[1],
                    "la": graph.meta.layer_distribution[2],
                    "pa": graph.meta.layer_distribution[3],
                    "chaos": graph.meta.layer_distribution[4],
                    "total": graph.meta.layer_distribution.iter().copied().sum::<usize>()
                })
            );

            user_success!("GEN_OK", "Topologie 3D extraite avec succès.");
        }

        SpatialCommands::Health => {
            user_info!("HEALTH", "Analyse de la stabilité des nœuds...");
            let graph = get_spatial_topology();

            // Identification des composants instables (stabilité < 0.5)
            let unstable_nodes: Vec<_> = graph.nodes.iter().filter(|n| n.stability < 0.5).collect();

            if unstable_nodes.is_empty() {
                user_success!("HEALTH_OK", "Stabilité nominale sur tous les nœuds.");
            } else {
                for node in unstable_nodes {
                    user_info!(
                        "GRAPH_VIBRATION_ALERT",
                        json!({
                            "node_id": node.label,
                            "stability": node.stability,
                            "is_critical": node.stability < 0.5,
                            "action": "check_convergence"
                        })
                    );
                }
            }
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_spatial_health_check() {
        let args = SpatialArgs {
            command: SpatialCommands::Health,
        };
        assert!(handle(args).await.is_ok());
    }
}
