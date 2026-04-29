// src-tauri/src/blockchain/p2p/vpn.rs
//! Pont entre le tunnel VPN souverain (Innernet) et le transport Libp2p.

use crate::blockchain::vpn::NetworkStatus;
use crate::utils::prelude::*;

/// Résolveur réseau pour mapper les IPs du VPN vers le format P2P.
pub struct P2PVpnResolver;

impl P2PVpnResolver {
    /// Génère la P2pMultiaddr d'écoute libp2p à partir du statut actuel du VPN.
    /// Si le VPN n'est pas connecté, on se replie sur localhost (127.0.0.1).
    pub fn get_listen_address(status: &NetworkStatus, port: u16) -> P2pMultiaddr {
        let ip = if status.connected {
            status.ip_address.as_deref().unwrap_or("127.0.0.1")
        } else {
            "127.0.0.1"
        };

        // Construction de la P2pMultiaddr formatée pour p2p
        let addr_str = format!("/ip4/{}/tcp/{}", ip, port);

        match addr_str.parse::<P2pMultiaddr>() {
            Ok(addr) => {
                if status.connected {
                    kernel_trace!(
                        "VPN Resolver",
                        &format!("Écoute P2P routée sur le tunnel souverain : {}", addr)
                    );
                }
                addr
            }
            Err(e) => {
                kernel_trace!(
                    "VPN Resolver",
                    &format!("Erreur de parsing IP ({}), repli sur localhost.", e)
                );
                "/ip4/127.0.0.1/tcp/0"
                    .parse()
                    .expect("Adresse de repli statique valide")
            }
        }
    }

    /// Vérifie si une IP appartient au sous-réseau souverain de Mentis (10.42.0.0/16).
    pub fn is_sovereign_addr(addr: &P2pMultiaddr) -> bool {
        addr.to_string().contains("/ip4/10.42.")
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::vpn::NetworkStatus;

    #[test]
    fn test_resolver_with_vpn_connected() {
        let status = NetworkStatus {
            connected: true,
            interface: "raise0".to_string(),
            ip_address: Some("10.42.0.1".to_string()),
            peers: vec![],
            uptime_seconds: None,
        };

        let addr = P2PVpnResolver::get_listen_address(&status, 4001);
        assert_eq!(addr.to_string(), "/ip4/10.42.0.1/tcp/4001");
        assert!(P2PVpnResolver::is_sovereign_addr(&addr));
    }

    #[test]
    fn test_resolver_fallback_on_disconnected() {
        let status = NetworkStatus {
            connected: false,
            interface: "raise0".to_string(),
            ip_address: None,
            peers: vec![],
            uptime_seconds: None,
        };

        let addr = P2PVpnResolver::get_listen_address(&status, 4001);
        assert!(addr.to_string().contains("127.0.0.1"));
        assert!(!P2PVpnResolver::is_sovereign_addr(&addr));
    }
}
