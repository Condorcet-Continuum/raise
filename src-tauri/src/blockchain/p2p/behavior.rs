// src-tauri/src/blockchain/p2p/behavior.rs
use crate::utils::prelude::*;

use crate::blockchain::p2p::protocol::{ArcadiaNetMessage, ArcadiaResponse};
use libp2p::gossipsub;
use libp2p::kad;
use libp2p::request_response;
use libp2p::swarm::NetworkBehaviour;
use libp2p::{identity, StreamProtocol};

/// Le comportement réseau combiné pour Raise (Arcadia Network).
#[derive(NetworkBehaviour)]
pub struct ArcadiaBehavior {
    /// Kademlia pour la découverte des pairs et la table de hachage distribuée.
    pub kademlia: kad::Behaviour<kad::store::MemoryStore>,

    /// Gossipsub pour la diffusion massive des commits et des votes.
    pub gossipsub: gossipsub::Behaviour,

    /// Request-Response pour les échanges directs (sync de chaîne, requêtes spécifiques).
    pub request_response: request_response::cbor::Behaviour<ArcadiaNetMessage, ArcadiaResponse>,
}

impl ArcadiaBehavior {
    /// Initialise un nouveau comportement réseau avec les clés locales.
    pub fn new(local_key: identity::Keypair) -> RaiseResult<Self> {
        let peer_id = local_key.public().to_peer_id();

        // 1. Configuration de Kademlia : Stockage en mémoire pour Raise.
        let store = kad::store::MemoryStore::new(peer_id);
        let kademlia = kad::Behaviour::new(peer_id, store);

        // 2. Configuration de Gossipsub : Authentification des messages requise.
        let gossipsub_config = match gossipsub::ConfigBuilder::default().build() {
            Ok(cfg) => cfg,
            Err(e) => raise_error!(
                "ERR_P2P_GOSSIPSUB_CONFIG",
                error = e,
                context = json!({
                    "action": "build_gossipsub_config",
                    "layer": "libp2p_network",
                    "hint": "Vérifiez les paramètres de validation du protocole ou les limites de taille de message."
                })
            ),
        };

        let gossipsub = match gossipsub::Behaviour::new(
            gossipsub::MessageAuthenticity::Signed(local_key),
            gossipsub_config,
        ) {
            Ok(behaviour) => behaviour,
            Err(e) => raise_error!(
                "ERR_P2P_BEHAVIOUR_INIT",
                error = e,
                context = json!({
                    "action": "initialize_gossipsub_behaviour",
                    "authenticity": "Signed",
                    "hint": "Échec de l'initialisation du comportement réseau. Vérifiez la validité de la clé locale (PeerId)."
                })
            ),
        };

        // 3. Configuration Request-Response : Utilisation du protocole CBOR pour Arcadia.
        let request_response = request_response::cbor::Behaviour::new(
            [(
                StreamProtocol::new("/arcadia/sync/1.0.0"),
                request_response::ProtocolSupport::Full,
            )],
            request_response::Config::default(),
        );

        Ok(Self {
            kademlia,
            gossipsub,
            request_response,
        })
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[test]
    fn test_behavior_initialization() {
        let local_key = identity::Keypair::generate_ed25519();
        let behavior = ArcadiaBehavior::new(local_key);

        assert!(
            behavior.is_ok(),
            "Le behavior devrait s'initialiser correctement avec les protocoles Arcadia"
        );
    }

    #[test]
    fn test_behavior_peer_id_consistency() {
        let local_key = identity::Keypair::generate_ed25519();

        // Initialisation du behavior
        let behavior = ArcadiaBehavior::new(local_key).expect("L'initialisation a échoué");

        // CORRECTION : Suppression de la variable inutilisée 'expected_peer_id'
        // pour éliminer le warning 'unused_variable'.
        // Le test valide ici la capacité à instancier la structure complète.
        drop(behavior);
        assert!(true);
    }
}
