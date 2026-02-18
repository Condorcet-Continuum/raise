use crate::blockchain::vpn::NetworkStatus;
use libp2p::Multiaddr;

use crate::utils::FromStr;
/// Pont entre le statut du VPN Mesh (Innernet) et la configuration réseau P2P.
pub struct P2PVpnResolver;

impl P2PVpnResolver {
    /// Génère la Multiaddr d'écoute libp2p à partir du statut actuel du VPN.
    /// Si le VPN n'est pas connecté, on se replie sur localhost (127.0.0.1).
    pub fn get_listen_address(status: &NetworkStatus, port: u16) -> Multiaddr {
        let ip = if status.connected {
            status.ip_address.as_deref().unwrap_or("127.0.0.1")
        } else {
            "127.0.0.1"
        };

        // Construction de la Multiaddr formatée pour libp2p
        match Multiaddr::from_str(&format!("/ip4/{}/tcp/{}", ip, port)) {
            Ok(addr) => addr,
            Err(_) => "/ip4/127.0.0.1/tcp/0"
                .parse()
                .expect("Valid fallback address"),
        }
    }

    /// Vérifie si une IP appartient au sous-réseau souverain de RAISE (10.42.0.0/16).
    pub fn is_sovereign_addr(addr: &Multiaddr) -> bool {
        addr.to_string().contains("/ip4/10.42.")
    }
}

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
    }
}
