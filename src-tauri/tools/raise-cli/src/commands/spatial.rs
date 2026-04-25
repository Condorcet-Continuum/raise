// FICHIER : src-tauri/tools/raise-cli/src/commands/spatial.rs

use clap::{Args, Subcommand};
use raise::{user_error, user_info, user_success, user_warn, utils::prelude::*}; // 🎯 Façade Unique RAISE

// Import de la logique spatiale du cœur
use raise::spatial_engine::get_spatial_topology;

// 🎯 Import du contexte global CLI
use crate::CliContext;

/// Pilotage du Spatial Engine (Visualisation 3D & Jumeau Numérique)
#[derive(Args, Clone, Debug)]
pub struct SpatialArgs {
    #[command(subcommand)]
    pub command: SpatialCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum SpatialCommands {
    /// Génère la topologie 3D actuelle et affiche les métadonnées de structure
    Topology,
    /// Identifie les composants présentant une instabilité physique (vibration/dérive)
    Health,
}

pub async fn handle(args: SpatialArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat de session
    if let Err(e) = ctx.session_mgr.touch().await {
        user_error!(
            "ERR_SESSION_HEARTBEAT",
            json_value!({"error": e.to_string()})
        );
    }

    match args.command {
        SpatialCommands::Topology => {
            user_info!(
                "SPATIAL_TOPOLOGY_GEN",
                json_value!({ "domain": ctx.active_domain })
            );
            let graph = get_spatial_topology();

            user_info!(
                "GRAPH_STATS",
                json_value!({
                    "nodes": graph.meta.node_count,
                    "total_elements": graph.meta.layer_distribution.iter().copied().sum::<usize>()
                })
            );

            user_success!(
                "SPATIAL_GEN_OK",
                json_value!({ "status": "topology_extracted" })
            );
        }

        SpatialCommands::Health => {
            user_info!(
                "SPATIAL_HEALTH_START",
                json_value!({ "action": "node_stability_analysis" })
            );

            let graph = get_spatial_topology();
            // Création de la liste des nœuds instables
            let unstable_nodes: Vec<_> = graph.nodes.iter().filter(|n| n.stability < 0.5).collect();

            if unstable_nodes.is_empty() {
                user_success!(
                    "SPATIAL_HEALTH_NOMINAL",
                    json_value!({ "status": "all_nodes_stable" })
                );
            } else {
                // 🎯 FIX : Utilisation de &unstable_nodes pour éviter le "move"
                for node in &unstable_nodes {
                    user_info!(
                        "SPATIAL_NODE_ALERT",
                        json_value!({
                            "id": node.label,
                            "stability": node.stability,
                            "critical": node.stability < 0.3
                        })
                    );
                }
                // 🎯 Désormais accessible sans erreur de borrow
                user_warn!(
                    "SPATIAL_HEALTH_WARNING",
                    json_value!({ "unstable_count": unstable_nodes.len() })
                );
            }
        }
    }
    Ok(())
}

// =========================================================================
// TESTS UNITAIRES (Conformité « Zéro Dette »)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use raise::utils::testing::DbSandbox;

    #[async_test]
    #[serial_test::serial]
    async fn test_spatial_health_workflow() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = crate::context::SessionManager::new(storage.clone());

        let ctx = crate::CliContext::mock(AppConfig::get(), session_mgr, storage);
        let args = SpatialArgs {
            command: SpatialCommands::Health,
        };

        handle(args, ctx).await
    }
}
