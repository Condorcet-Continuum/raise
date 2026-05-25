// src-tauri/src/blockchain/p2p/behavior.rs
//! Comportement réseau p2p (Behaviour) combinant Kademlia, Gossipsub et Request-Response.

use crate::blockchain::p2p::protocol::{MentisNetMessage, MentisResponse};
use crate::utils::prelude::*;

/// Définition de la stack comportementale de Mentis.
#[derive(P2pBehaviour)]
pub struct MentisBehavior {
    pub kademlia: P2pKademlia::Behaviour<P2pKademlia::store::MemoryStore>,
    pub gossipsub: P2pGossipSub::Behaviour,
    pub request_response: P2pRequestResponse::cbor::Behaviour<MentisNetMessage, MentisResponse>,
    pub limits: P2pConnectionLimits::Behaviour,
}

impl MentisBehavior {
    /// Initialise la stack comportementale du réseau Mentis.
    pub fn new(local_key: P2pIdentity::Keypair) -> RaiseResult<Self> {
        let peer_id = local_key.public().to_peer_id();

        // 1. Kademlia : Pour la découverte et le routage des pairs
        let store = P2pKademlia::store::MemoryStore::new(peer_id);

        // On passe directement notre protocole privé au constructeur `new`.
        let kad_config = P2pKademlia::Config::new(P2pStreamProtocol::new("/mentis/kad/1.0.0"));
        let kademlia = P2pKademlia::Behaviour::with_config(peer_id, store, kad_config);

        // 2. GossipSub : Pour la diffusion rapide des blocs et des votes
        let g_config: P2pGossipSub::Config = match P2pGossipSub::ConfigBuilder::default()
            // 🎯 FIX PERFORMANCE : On autorise des messages jusqu'à 10 Mo pour les gros commits
            .max_transmit_size(10 * 1024 * 1024)
            .build()
        {
            Ok(c) => c,
            Err(e) => raise_error!("ERR_P2P_GOSSIP_CONFIG", error = e.to_string()),
        };

        let gossipsub = match P2pGossipSub::Behaviour::new(
            P2pGossipSub::MessageAuthenticity::Signed(local_key.clone()), // .clone() peut être requis selon l'impl de la clé
            g_config,
        ) {
            Ok(b) => b,
            Err(e) => raise_error!("ERR_P2P_GOSSIP_INIT", error = e.to_string()),
        };

        // 3. Request-Response : Pour les requêtes ciblées (Synchronisation d'un bloc manquant)
        let request_response = P2pRequestResponse::cbor::Behaviour::new(
            [(
                P2pStreamProtocol::new("/mentis/sync/1.0.0"),
                P2pRequestResponse::ProtocolSupport::Full,
            )],
            P2pRequestResponse::Config::default(),
        );

        // 4. Connection Limits : Protection Anti-DDoS basique
        let limits = P2pConnectionLimits::Behaviour::new(
            P2pConnectionLimits::ConnectionLimits::default()
                .with_max_pending_incoming(Some(50))
                .with_max_pending_outgoing(Some(50))
                .with_max_established_incoming(Some(100))
                .with_max_established_outgoing(Some(100)),
        );

        Ok(Self {
            kademlia,
            gossipsub,
            request_response,
            limits,
        })
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mentis_behavior_init_robustness() {
        // Utilisation de l'alias d'identité du prelude
        let key = P2pIdentity::Keypair::generate_ed25519();
        let behavior = MentisBehavior::new(key);
        assert!(
            behavior.is_ok(),
            "Le comportement réseau doit s'initialiser sans erreur"
        );
    }
}
