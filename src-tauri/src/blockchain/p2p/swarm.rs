// src-tauri/src/blockchain/p2p/swarm.rs
//! Configuration et assemblage du Swarm p2p (Transport + Sécurité + Multiplexage).
//! Récrit pour utiliser exclusivement la façade (prelude) du projet Raise.

use crate::blockchain::p2p::behavior::MentisBehavior;
use crate::utils::prelude::*;

/// Crée et configure le Swarm réseau pour le nœud local.
pub async fn create_swarm(
    local_key: P2pIdentity::Keypair,
) -> RaiseResult<P2pSwarm<MentisBehavior>> {
    // 1. Initialisation de la stack comportementale (Kademlia, Gossipsub, etc.)
    let behavior = MentisBehavior::new(local_key.clone())?;

    // 2. Configuration du transport (TCP sécurisé et multiplexé)
    // L'utilisation de `with_existing_identity` et `with_tokio` est la norme absolue pour p2p moderne.
    let transport = match P2pSwarmBuilder::with_existing_identity(local_key)
        .with_tokio()
        .with_tcp(
            Default::default(),
            P2pNoise::Config::new, // Résolution absolue (comme dans utils/network/p2p.rs)
            P2pYamux::Config::default, // Résolution absolue
        ) {
        Ok(t) => t,
        Err(e) => raise_error!("ERR_P2P_TRANSPORT", error = e.to_string()),
    };

    // 3. Assemblage final
    let swarm = match transport.with_behaviour(|_| behavior) {
        Ok(b) => b.build(),
        Err(e) => raise_error!("ERR_P2P_SWARM_BUILD", error = e.to_string()),
    };

    Ok(swarm)
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_mentis_swarm_creation_robustness() {
        // 🎯 FIX: Utilisation de l'identité via la façade
        let key = P2pIdentity::Keypair::generate_ed25519();
        assert!(
            create_swarm(key).await.is_ok(),
            "Le Swarm doit s'initialiser correctement avec la stack Mentis"
        );
    }
}
