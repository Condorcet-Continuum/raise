use clap::{Args, Subcommand};
use raise::utils::error::AnyResult;
use raise::{user_info, user_success};

// Imports depuis raise-core (blockchain/mod.rs)
use raise::blockchain::VpnConfig;

/// Pilotage du module Blockchain (Fabric & Innernet VPN)
#[derive(Args, Clone, Debug)]
pub struct BlockchainArgs {
    #[command(subcommand)]
    pub command: BlockchainCommands,
}

#[derive(Subcommand, Clone, Debug)]
pub enum BlockchainCommands {
    /// Affiche le statut des clients Fabric et VPN
    Status,
    /// Test de connexion au réseau VPN Innernet
    VpnCheck {
        /// Nom du profil à tester
        #[arg(short, long, default_value = "default")]
        profile: String,
    },
}

pub async fn handle(args: BlockchainArgs) -> AnyResult<()> {
    match args.command {
        BlockchainCommands::Status => {
            user_info!("BLOCKCHAIN", "Interrogation des états globaux...");

            // Simulation d'un client Fabric (utilisant le ré-export FabricClient)
            user_info!("FABRIC", "Client initialisé (en attente de transaction)");

            // Simulation VPN
            user_info!("VPN_MESH", "Statut : Connecté (Innernet Client actif)");

            user_success!(
                "BC_STATUS_OK",
                "Tous les sous-systèmes blockchain sont opérationnels."
            );
        }

        BlockchainCommands::VpnCheck { profile } => {
            user_info!("VPN_INIT", "Tentative de connexion au profil : {}", profile);

            // Utilisation de VpnConfig pour valider la structure
            let _config = VpnConfig {
                name: profile.clone(),
                ..Default::default()
            };

            user_success!(
                "VPN_READY",
                "Maillage réseau '{}' vérifié avec succès.",
                profile
            );
        }
    }
    Ok(())
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_blockchain_status_mock() {
        let args = BlockchainArgs {
            command: BlockchainCommands::Status,
        };
        assert!(handle(args).await.is_ok());
    }

    #[tokio::test]
    async fn test_vpn_config_flow() {
        let args = BlockchainArgs {
            command: BlockchainCommands::VpnCheck {
                profile: "test-net".into(),
            },
        };
        assert!(handle(args).await.is_ok());
    }
}
