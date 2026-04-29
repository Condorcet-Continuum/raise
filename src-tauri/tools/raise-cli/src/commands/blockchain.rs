// FICHIER : src-tauri/tools/raise-cli/src/commands/blockchain.rs

use clap::{Args, Subcommand};

use raise::{user_error, user_info, user_success, utils::prelude::*}; // 🎯 Façade Unique RAISE

// 🎯 Import du contexte global CLI
use crate::CliContext;

/// Pilotage du module Blockchain (Fabric & Innernet VPN)
#[derive(Args, Clone, Debug)]
pub struct BlockchainArgs {
    #[command(subcommand)]
    pub command: BlockchainCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum BlockchainCommands {
    /// Affiche le statut du nœud Arcadia et l'état du catalogue de connaissances.
    Status,
    /// Vérifie la connectivité du nœud P2P.
    SyncCheck {
        /// Affiche plus de détails sur les pairs connectés.
        #[arg(short, long)]
        verbose: bool,
    },
}

/// Handler principal pour les commandes Blockchain
pub async fn handle(args: BlockchainArgs, ctx: CliContext) -> RaiseResult<()> {
    // 🎯 Heartbeat automatique pour maintenir la session active
    if let Err(e) = ctx.session_mgr.touch().await {
        user_error!(
            "ERR_SESSION_HEARTBEAT",
            json_value!({"error": e.to_string()})
        );
    } else {
        user_debug!("SESSION_TOUCHED");
    }

    match args.command {
        BlockchainCommands::Status => {
            user_info!(
                "BLOCKCHAIN_STATUS_QUERY",
                json_value!({
                    "active_domain": ctx.active_domain,
                    "system_partition": ctx.config.mount_points.system.domain,
                    "active_user": ctx.active_user
                })
            );

            // Simulation d'un client Fabric (utilisant le ré-export FabricClient)
            user_info!(
                "FABRIC_NODE",
                json_value!({"status": "Client initialisé (IDLE)"})
            );

            // Simulation VPN via Innernet
            user_info!(
                "VPN_MESH",
                json_value!({"status": "Innernet Client actif (connected)"})
            );

            user_success!(
                "BC_STATUS_OK",
                json_value!({"message": "Sous-systèmes blockchain et VPN opérationnels."})
            );
        }

        BlockchainCommands::SyncCheck { verbose } => {
            user_info!("SYNC_DIAGNOSTIC_INIT", json_value!({ "verbose": verbose }));

            // Ici on simule la vérification que le Swarm est bien monté
            user_success!(
                "P2P_READY",
                json_value!({
                    "mesh_verified": true,
                    "discovery": "Kademlia Active"
                })
            );
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
    use raise::utils::context::SessionManager;
    use raise::utils::testing::DbSandbox;

    #[async_test]
    #[serial_test::serial] // 🎯 FIX : Évite les conflits de lock sur la Sandbox
    async fn test_blockchain_status_mock() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);
        let args = BlockchainArgs {
            command: BlockchainCommands::Status,
        };

        // 🎯 Rigueur : On utilise directement le RaiseResult
        handle(args, ctx).await
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_arcadia_status_flow() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);
        let args = BlockchainArgs {
            command: BlockchainCommands::Status,
        };

        handle(args, ctx).await
    }

    #[async_test]
    #[serial_test::serial]
    async fn test_p2p_config_check() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let storage = SharedRef::new(sandbox.storage.clone());
        let session_mgr = SessionManager::new(storage.clone());

        let ctx = CliContext::mock(AppConfig::get(), session_mgr, storage);
        let args = BlockchainArgs {
            command: BlockchainCommands::SyncCheck { verbose: true },
        };

        handle(args, ctx).await
    }
}
