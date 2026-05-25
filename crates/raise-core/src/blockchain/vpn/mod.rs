// src-tauri/src/blockchain/vpn/mod.rs
//! Sous-module VPN Mentis : Orchestre le maillage réseau souverain via Innernet et WireGuard.

/// Client d'orchestration pour la CLI Innernet.
pub mod innernet_client;

// =========================================================================
// FAÇADE VPN (Standard RAISE)
// =========================================================================
// Réexportations stratégiques pour isoler le reste du projet des détails
// d'implémentation de la CLI.

pub use innernet_client::{InnernetClient, NetworkConfig, NetworkStatus, Peer};

// =========================================================================
// TESTS DE CONFORMITÉ DE LA FAÇADE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Vérifie la visibilité et la cohérence des types via la façade.
    #[test]
    fn test_vpn_facade_integrity() {
        let _config = NetworkConfig::default();

        let status = NetworkStatus {
            connected: false,
            interface: "raise0".into(),
            ip_address: None,
            peers: Vec::new(),
            uptime_seconds: None,
        };

        assert!(!status.connected);
        assert_eq!(status.interface, "raise0");
    }
}
