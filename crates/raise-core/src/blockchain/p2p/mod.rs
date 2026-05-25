// src-tauri/src/blockchain/p2p/mod.rs
//! Sous-module P2P Mentis : Gère le transport, le comportement réseau et l'orchestration.

pub mod behavior;
pub mod protocol;
pub mod service;
pub mod swarm;
pub mod vpn;

// Réexportations stratégiques pour simplifier l'usage par les commandes Tauri

pub use behavior::MentisBehavior;
pub use protocol::{MentisNetMessage, MentisResponse};
pub use service::{init_mentis_network, spawn_p2p_service};
pub use swarm::create_swarm;
pub use vpn::P2PVpnResolver; // 🎯 Rendu accessible pour la configuration réseau

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::prelude::*;

    /// Test de visibilité et de cohérence des types réexportés.
    #[test]
    fn test_p2p_module_visibility() {
        let response = MentisResponse::CommitNotFound;

        match response {
            MentisResponse::CommitNotFound => assert!(true),
            _ => panic!("Type MentisResponse mal exporté ou incohérent."),
        }
    }

    /// Test de présence des composants fondamentaux.
    #[test]
    fn test_p2p_components_availability() {
        let _message_type = VariantMarker(&MentisNetMessage::RequestLatestHash);
        assert!(true);
    }
}
