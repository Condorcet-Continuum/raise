// FICHIER : src-tauri/tools/raise-cli/src/commands/spatial.rs

use clap::{Args, Subcommand};

use raise::{user_info, user_success, utils::prelude::*};

// Import de la fonction principale de topologie
use raise::spatial_engine::get_spatial_topology;

// 🎯 NOUVEAU : Import du contexte global CLI
use crate::CliContext;

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

// 🎯 La signature intègre le CliContext
pub async fn handle(args: SpatialArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique
    let _ = ctx.session_mgr.touch().await;

    match args.command {
        SpatialCommands::Topology => {
            // 🎯 Mise en conformité stricte JSON
            user_info!(
                "SPATIAL_START",
                json_value!({"action": "Génération procédurale de la topologie Arcadia..."})
            );

            // Récupération du graphe spatial
            let graph = get_spatial_topology();

            user_info!(
                "GRAPH_STATS_NODES",
                json_value!({
                    "count": graph.meta.node_count,
                    "complexity": if graph.meta.node_count > 1000 { "high" } else { "standard" }
                })
            );

            // Accès aux statistiques par couche (OA, SA, LA, PA, Chaos)
            user_info!(
                "GRAPH_LAYERS_DISTRIBUTION",
                json_value!({
                    "oa": graph.meta.layer_distribution[0],
                    "sa": graph.meta.layer_distribution[1],
                    "la": graph.meta.layer_distribution[2],
                    "pa": graph.meta.layer_distribution[3],
                    "chaos": graph.meta.layer_distribution[4],
                    "total": graph.meta.layer_distribution.iter().copied().sum::<usize>()
                })
            );

            // 🎯 Payload JSON pour le succès
            user_success!(
                "GEN_OK",
                json_value!({"status": "Topologie 3D extraite avec succès."})
            );
        }

        SpatialCommands::Health => {
            // 🎯 Mise en conformité stricte JSON
            user_info!(
                "HEALTH_START",
                json_value!({"action": "Analyse de la stabilité des nœuds..."})
            );

            let graph = get_spatial_topology();

            // Identification des composants instables (stabilité < 0.5)
            let unstable_nodes: Vec<_> = graph.nodes.iter().filter(|n| n.stability < 0.5).collect();

            if unstable_nodes.is_empty() {
                // 🎯 Payload JSON pour le succès
                user_success!(
                    "HEALTH_OK",
                    json_value!({"status": "Stabilité nominale sur tous les nœuds."})
                );
            } else {
                for node in unstable_nodes {
                    user_info!(
                        "GRAPH_VIBRATION_ALERT",
                        json_value!({
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
    use crate::CliContext;
    use raise::utils::context::SessionManager;

    #[cfg(test)]
    use raise::utils::testing::DbSandbox;

    #[async_test]
    async fn test_spatial_health_check() {
        // 🎯 On simule le contexte global pour le test
        let sandbox = DbSandbox::new().await;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext {
            config: AppConfig::get(),
            session_mgr,
            storage,
        };

        let args = SpatialArgs {
            command: SpatialCommands::Health,
        };

        assert!(handle(args, ctx).await.is_ok());
    }
}
