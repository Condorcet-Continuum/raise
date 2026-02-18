// src-tauri/src/blockchain/mod.rs
//! Point d'entrée du module Blockchain.
//!
//! Agit comme une façade pour :
//! 1. La gestion des erreurs unifiée via `error::BlockchainError`.
//! 2. Le client Hyperledger Fabric réel (`fabric`).
//! 3. Le client VPN Innernet réel (`vpn`).
//! 4. La gestion de l'état global (State) pour l'application Tauri.

use crate::utils::{HashMap, Mutex};
use tauri::{AppHandle, Manager, Runtime, State};

// Exposition publique des sous-modules
pub mod bridge;
pub mod consensus;
pub mod crypto;
pub mod error;
pub mod fabric;
pub mod p2p;
pub mod storage;
pub mod sync;
pub mod vpn;

// Réexportations stratégiques pour le moteur Arcadia
pub use consensus::ArcadiaConsensus;
pub use p2p::swarm::create_swarm;
pub use storage::chain::Ledger;
pub use storage::commit::ArcadiaCommit;
pub use sync::SyncEngine;

// --- RÉ-EXPORTS ---

pub use self::fabric::client::FabricClient;
pub use self::fabric::config::ConnectionProfile;

// On ré-exporte le client VPN et sa config (NetworkConfig renommé en VpnConfig pour la clarté)
pub use self::vpn::innernet_client::{InnernetClient, NetworkConfig as VpnConfig};

// =============================================================================
//  GESTION DES ÉTATS TAURI (SHARED STATE)
// =============================================================================

/// Type alias pour le client Fabric partagé
pub type SharedFabricClient = Mutex<FabricClient>;
/// Type alias pour le client VPN partagé
pub type SharedInnernetClient = Mutex<InnernetClient>;

// --- HELPERS D'ACCÈS ---

/// Helper pour récupérer le client Innernet depuis une commande Tauri.
pub fn innernet_state<R: Runtime>(app: &AppHandle<R>) -> State<'_, SharedInnernetClient> {
    app.state::<SharedInnernetClient>()
}

/// Helper pour récupérer le client Fabric depuis une commande Tauri.
pub fn fabric_state<R: Runtime>(app: &AppHandle<R>) -> State<'_, SharedFabricClient> {
    app.state::<SharedFabricClient>()
}

// --- INITIALISATION ---

/// Initialise le client Innernet dans le state Tauri.
pub fn ensure_innernet_state<R: Runtime>(app: &AppHandle<R>, default_profile: impl Into<String>) {
    if app.try_state::<SharedInnernetClient>().is_none() {
        let profile_name = default_profile.into();

        // CORRECTION CLIPPY : Initialisation directe au lieu de reassignment
        let vpn_config = VpnConfig {
            name: profile_name.clone(),
            ..Default::default()
        };

        let client = InnernetClient::new(vpn_config);

        tracing::info!(
            "🔒 [Blockchain] Initialisation State Innernet (profil: {})",
            profile_name
        );
        app.manage(Mutex::new(client));
    }
}

/// Initialise un client Fabric vide (en attente de chargement de profil).
pub fn ensure_fabric_state<R: Runtime>(app: &AppHandle<R>) {
    if app.try_state::<SharedFabricClient>().is_none() {
        let empty_profile = ConnectionProfile {
            name: "pending".into(),
            version: "1.0".into(),
            client: self::fabric::config::ClientConfig {
                organization: "unknown".into(),
                connection: None,
            },
            organizations: HashMap::new(),
            peers: HashMap::new(),
            certificate_authorities: HashMap::new(),
        };

        let client = FabricClient::from_config(empty_profile);
        app.manage(Mutex::new(client));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpn_config_initialization() {
        // Test de la logique d'initialisation propre
        let config = VpnConfig {
            name: "test-mesh".to_string(),
            ..Default::default()
        };
        assert_eq!(config.name, "test-mesh");
    }

    #[test]
    fn test_fabric_client_reexport() {
        let profile = ConnectionProfile {
            name: "test".into(),
            version: "1.0".into(),
            client: self::fabric::config::ClientConfig {
                organization: "Org1".into(),
                connection: None,
            },
            organizations: HashMap::new(),
            peers: HashMap::new(),
            certificate_authorities: HashMap::new(),
        };
        let _client = FabricClient::from_config(profile);
    }
}
