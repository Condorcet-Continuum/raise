// src-tauri/src/blockchain/p2p/swarm.rs
use crate::utils::prelude::*;

use crate::blockchain::p2p::behavior::ArcadiaBehavior;
use crate::utils::Duration;
use libp2p::{identity, noise, tcp, yamux, Swarm, SwarmBuilder};

/// Crée et configure un Swarm libp2p pour le réseau Raise.
/// Le Swarm combine le transport (TCP + Noise + Yamux) et le comportement (ArcadiaBehavior).
pub async fn create_swarm(local_key: identity::Keypair) -> RaiseResult<Swarm<ArcadiaBehavior>> {
    // Initialisation du comportement Arcadia (Kademlia + Gossipsub + ReqResp)
    let behavior = ArcadiaBehavior::new(local_key.clone())?;

    // Construction du Swarm avec la pile technologique Arcadia
    // 1. Configuration du Transport (TCP + Noise + Yamux)
    let swarm_with_transport = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        );

    let transport_builder = match swarm_with_transport {
        Ok(builder) => builder,
        Err(e) => raise_error!(
            "ERR_P2P_TRANSPORT_CONFIG",
            error = e,
            context = json!({
                "action": "build_p2p_transport",
                "stack": "TCP/Noise/Yamux",
                "hint": "Échec de la configuration de la pile de transport. Vérifiez les dépendances Noise/Yamux."
            })
        ),
    };

    // 2. Injection du Behaviour et Build final
    let swarm = match transport_builder.with_behaviour(|_| behavior) {
        Ok(builder) => {
            // On configure et on build ici
            builder
                .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
                .build()
        }
        Err(e) => raise_error!(
            "ERR_P2P_SWARM_BEHAVIOUR_INJECTION",
            error = e,
            context = json!({
                "action": "inject_behaviour_into_swarm",
                "component": "NetworkBehavior",
                "hint": "L'injection du comportement réseau a échoué. Vérifiez la compatibilité des protocoles sélectionnés."
            })
        ),
    };

    Ok(swarm)
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use libp2p::identity;

    #[tokio::test]
    async fn test_swarm_creation() {
        let local_key = identity::Keypair::generate_ed25519();
        let swarm_result = create_swarm(local_key).await;

        assert!(
            swarm_result.is_ok(),
            "Le Swarm devrait être créé sans erreur avec la pile Arcadia (TCP/Noise/Yamux)"
        );

        // CORRECTION : Suppression du 'mut' car l'instance du swarm n'est pas modifiée.
        let swarm = swarm_result.expect("Échec de l'obtention du Swarm");

        // Vérification que le PeerId local est correctement dérivé
        let peer_id_str = swarm.local_peer_id().to_string();
        assert!(!peer_id_str.is_empty());
    }
}
