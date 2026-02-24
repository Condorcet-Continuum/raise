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
    let swarm = SwarmBuilder::with_existing_identity(local_key)
        .with_tokio() // Utilisation du runtime Tokio comme configuré dans le Cargo.toml
        .with_tcp(
            tcp::Config::default(),
            noise::Config::new,
            yamux::Config::default,
        )
        .map_err(|e| format!("Erreur lors de la configuration du transport: {:?}", e))?
        .with_behaviour(|_| behavior)
        .map_err(|e| format!("Erreur lors de l'ajout du behavior: {:?}", e))?
        .with_swarm_config(|cfg| cfg.with_idle_connection_timeout(Duration::from_secs(60)))
        .build();

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
