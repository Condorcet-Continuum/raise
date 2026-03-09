// FICHIER : src-tauri/src/utils/network/p2p.rs
// 1. Core : Erreurs
use crate::utils::core::error::RaiseResult;

// 2. Data : JSON
use crate::utils::data::json::json_value;

// 3. Network : Types P2P (via la façade network/mod.rs)
use crate::utils::network::p2p_types::{
    P2pIdentity, P2pMultiaddr, P2pNetworkBehaviourTrait, P2pPeerId, P2pSwarm,
};

/// Construit et configure un nœud P2P prêt à être démarré.
/// 🤖 IA NOTE : Ne gère pas la boucle d'événements (Swarm Event Loop).
/// Retourne le Swarm configuré pour être piloté par le service Blockchain.
pub async fn build_p2p_node_async<B: P2pNetworkBehaviourTrait>(
    behaviour: B,
    keypair: P2pIdentity::Keypair,
    listen_port: u16,
) -> RaiseResult<P2pSwarm<B>> {
    let peer_id = P2pPeerId::from(keypair.public());

    crate::user_debug!(
        "NETWORK_P2P_INIT",
        json_value!({ "peer_id": peer_id.to_string(), "port": listen_port })
    );

    // Initialisation du Swarm avec TCP, Noise (Sec) et Yamux (Mux)
    let mut swarm = match libp2p::SwarmBuilder::with_existing_identity(keypair)
        .with_tokio()
        .with_tcp(
            Default::default(),
            libp2p::noise::Config::new,
            libp2p::yamux::Config::default,
        ) {
        Ok(builder) => match builder.with_behaviour(|_| behaviour) {
            Ok(b) => b.build(),
            Err(e) => crate::raise_error!("ERR_NETWORK_P2P_BEHAVIOUR", error = e),
        },
        Err(e) => crate::raise_error!("ERR_NETWORK_P2P_TRANSPORT", error = e),
    };

    // Configuration de l'écoute avec match explicite
    let listen_addr_str = format!("/ip4/0.0.0.0/tcp/{}", listen_port);
    let listen_addr: P2pMultiaddr = match listen_addr_str.parse() {
        Ok(addr) => addr,
        Err(e) => crate::raise_error!("ERR_NETWORK_P2P_ADDR_PARSE", error = e),
    };

    if let Err(e) = swarm.listen_on(listen_addr.clone()) {
        crate::raise_error!(
            "ERR_NETWORK_P2P_LISTEN",
            error = e,
            context = json_value!({ "address": listen_addr.to_string() })
        );
    }

    Ok(swarm)
}
