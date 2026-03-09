// src-tauri/src/blockchain/vpn/innernet_client.rs
//! Client Innernet pour RAISE
//!
//! Gère la connexion au mesh VPN WireGuard via la CLI `innernet`.
//! Utilise tokio::process pour ne pas bloquer le runtime Tauri.

use crate::utils::prelude::*;

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct NetworkConfig {
    pub name: String,
    pub cidr: String,
    pub server_endpoint: String,
    pub interface: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            name: "raise".to_string(),
            cidr: "10.42.0.0/16".to_string(),
            server_endpoint: "vpn.raise.local:51820".to_string(),
            interface: "raise0".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct Peer {
    pub name: String,
    pub ip: String,
    pub public_key: String,
    pub endpoint: Option<String>,
    pub last_handshake: Option<i64>,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
}

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct NetworkStatus {
    pub connected: bool,
    pub interface: String,
    pub ip_address: Option<String>,
    pub peers: Vec<Peer>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Clone)]
pub struct InnernetClient {
    config: NetworkConfig,
    status: SharedRef<AsyncRwLock<NetworkStatus>>,
}

impl InnernetClient {
    /// Crée une nouvelle instance du client Innernet
    pub fn new(config: NetworkConfig) -> Self {
        let status = NetworkStatus {
            connected: false,
            interface: config.interface.clone(),
            ip_address: None,
            peers: Vec::new(),
            uptime_seconds: None,
        };

        Self {
            config,
            status: SharedRef::new(AsyncRwLock::new(status)),
        }
    }

    /// Vérifie si Innernet est installé (Appel bloquant acceptable au démarrage)
    // 🎯 MIGRATION : Utilisation de RaiseResult
    pub fn check_installation() -> RaiseResult<String> {
        let output = match ProcessCommand::new("innernet").arg("--version").output() {
            Ok(out) => out,
            Err(e) => raise_error!(
                "ERR_VPN_INNERNET_MISSING",
                error = e,
                context = json_value!({
                    "action": "check_innernet_installation",
                    "command": "innernet --version",
                    "hint": "Le binaire 'innernet' est introuvable. Assurez-vous qu'il est installé (sudo apt install innernet) et que l'utilisateur courant a les droits d'exécution."
                })
            ),
        };

        if !output.status.success() {
            crate::raise_error!(
                "ERR_VPN_INNERNET_EXEC_FAIL",
                error = "L'exécution de la commande Innernet a échoué.",
                context = json_value!({
                    "action": "check_innernet_installation",
                    "status_code": output.status.code(),
                    "stderr": String::from_utf8_lossy(&output.stderr).to_string()
                })
            );
        }

        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version.trim().to_string())
    }

    /// Se connecte au réseau mesh (Async)
    pub async fn connect(&self) -> RaiseResult<()> {
        tracing::info!("Connecting to Innernet network: {}", self.config.name);

        let output = self.run_command(["up", &self.config.name]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            crate::raise_error!(
                "ERR_VPN_CONNECTION_FAIL",
                context = json_value!({
                    "network_name": self.config.name,
                    "action": "connect_vpn",
                    "status_code": output.status.code(),
                    "stderr": stderr.trim(),
                    "hint": "La commande a été trouvée mais a échoué. Vérifiez vos privilèges sudo, l'existence de l'interface ou si le serveur Innernet est joignable."
                })
            );
        }

        // Mettre à jour le statut
        let mut status = self.status.write().await;
        status.connected = true;

        // On relâche le lock avant d'appeler get_interface_ip qui est async
        drop(status);

        if let Ok(ip) = self.get_interface_ip().await {
            let mut status = self.status.write().await;
            status.ip_address = Some(ip);
        }

        tracing::info!("Successfully connected to {}", self.config.name);

        Ok(())
    }

    /// Se déconnecte du réseau mesh (Async)
    pub async fn disconnect(&self) -> RaiseResult<()> {
        tracing::info!("Disconnecting from Innernet network: {}", self.config.name);

        let output = self.run_command(["down", &self.config.name]).await?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            raise_error!(
                "ERR_VPN_DISCONNECTION_FAIL",
                context = json_value!({
                    "network_name": self.config.name,
                    "action": "disconnect_vpn",
                    "status_code": output.status.code(),
                    "stderr": stderr.trim(),
                    "hint": "Impossible de fermer l'interface VPN. Vérifiez si le processus innernet est verrouillé ou si l'interface a déjà été supprimée manuellement."
                })
            );
        }

        // Mettre à jour le statut
        let mut status = self.status.write().await;
        status.connected = false;
        status.ip_address = None;
        status.peers.clear();

        tracing::info!("Successfully disconnected from {}", self.config.name);

        Ok(())
    }

    /// Récupère le statut actuel du réseau
    pub async fn get_status(&self) -> RaiseResult<NetworkStatus> {
        // Tentative de mise à jour des peers si possible
        if let Ok(peers) = self.fetch_peers().await {
            let mut status = self.status.write().await;
            status.peers = peers;
            if !status.peers.is_empty() {
                status.connected = true;
            }
        }

        Ok(self.status.read().await.clone())
    }

    /// Liste tous les peers du réseau
    pub async fn list_peers(&self) -> RaiseResult<Vec<Peer>> {
        self.fetch_peers().await
    }

    /// Ajoute un nouveau peer via un code d'invitation
    pub async fn add_peer(&self, _invitation_code: &str) -> RaiseResult<String> {
        tracing::info!("Adding peer with invitation code");
        // TODO: Implémentation réelle avec fichier temporaire pour l'invitation
        Ok("Peer added successfully (Simulation)".to_string())
    }

    /// Exécute une commande Innernet (Async)
    async fn run_command<I, S>(&self, args: I) -> RaiseResult<ProcessOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<std::ffi::OsStr>,
    {
        // 1. Exécution et capture de l'erreur système
        let output = match AsyncCommand::new("innernet").args(args).output().await {
            Ok(out) => out,
            Err(e) => {
                raise_error!(
                    "ERR_VPN_COMMAND_EXEC",
                    error = e,
                    context = json_value!({
                        "action": "execute_innernet_cli",
                        "hint": "Le binaire innernet n'a pas pu être lancé. Vérifiez qu'il est dans le PATH."
                    })
                )
            }
        };

        // 2. Vérification du succès de la commande (Exit Code)
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            raise_error!(
                "ERR_VPN_COMMAND_FAILED",
                context = json_value!({
                    "exit_code": output.status.code(),
                    "stderr": stderr,
                    "action": "validate_innernet_execution",
                    "hint": "Innernet a renvoyé une erreur. Vérifiez les logs stderr ci-dessus."
                })
            )
        }

        Ok(output)
    }

    /// Récupère l'IP de l'interface
    async fn get_interface_ip(&self) -> RaiseResult<String> {
        let output = self.run_command(["show", &self.config.name]).await?;

        if !output.status.success() {
            crate::raise_error!(
                "ERR_VPN_INTERFACE_DOWN",
                error = "Impossible de récupérer les informations de l'interface (Interface probablement inactive).",
                context = json_value!({
                    "network_name": self.config.name,
                    "action": "get_interface_ip"
                })
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parser la sortie pour extraire l'IP
        for line in stdout.lines() {
            if line.contains("ip:") {
                if let Some(ip_part) = line.split("ip:").nth(1) {
                    let ip = ip_part.trim().split('/').next().unwrap_or("");
                    if !ip.is_empty() {
                        return Ok(ip.to_string());
                    }
                }
            }
        }

        crate::raise_error!(
            "ERR_VPN_IP_PARSE_FAIL",
            error = "Impossible d'extraire l'adresse IP de la sortie Innernet.",
            context = json_value!({
                "network_name": self.config.name,
                "action": "parse_interface_ip",
                "raw_output": stdout.to_string()
            })
        );
    }

    /// Récupère la liste des peers via WireGuard
    async fn fetch_peers(&self) -> RaiseResult<Vec<Peer>> {
        let output = match AsyncCommand::new("wg")
            .args(["show", &self.config.interface])
            .output()
            .await
        {
            Ok(out) => out,
            Err(e) => raise_error!(
                "ERR_VPN_WG_COMMAND_FAIL",
                error = e,
                context = json_value!({
                    "interface": self.config.interface,
                    "action": "fetch_wireguard_peers",
                    "hint": "La commande 'wg' a échoué. Vérifiez que WireGuard est installé et que l'utilisateur a les droits 'sudo' ou les capacités CAP_NET_ADMIN."
                })
            ),
        };

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        self.parse_wg_output(&stdout)
    }

    /// Parse la sortie de `wg show`
    fn parse_wg_output(&self, output: &str) -> RaiseResult<Vec<Peer>> {
        let mut peers = Vec::new();
        let mut current_peer: Option<Peer> = None;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("peer:") {
                if let Some(peer) = current_peer.take() {
                    peers.push(peer);
                }

                let public_key = line.split_whitespace().nth(1).unwrap_or("").to_string();

                current_peer = Some(Peer {
                    name: "unknown".to_string(),
                    ip: "0.0.0.0".to_string(),
                    public_key,
                    endpoint: None,
                    last_handshake: None,
                    transfer_rx: 0,
                    transfer_tx: 0,
                });
            } else if let Some(ref mut peer) = current_peer {
                if line.starts_with("endpoint:") {
                    peer.endpoint = line.split_whitespace().nth(1).map(String::from);
                } else if line.starts_with("allowed ips:") {
                    if let Some(ips) = line.split(':').nth(1) {
                        if let Some(first_ip) = ips.split(',').next() {
                            peer.ip = first_ip
                                .trim()
                                .split('/')
                                .next()
                                .unwrap_or("0.0.0.0")
                                .to_string();
                        }
                    }
                } else if line.starts_with("latest handshake:") {
                    peer.last_handshake = Some(UtcClock::now().timestamp());
                }
            }
        }

        if let Some(peer) = current_peer {
            peers.push(peer);
        }

        Ok(peers)
    }

    /// Ping un peer spécifique
    pub async fn ping_peer(&self, peer_ip: &str) -> RaiseResult<bool> {
        let output = match AsyncCommand::new("ping")
            .args(["-c", "1", "-W", "2", peer_ip])
            .output()
            .await
        {
            Ok(out) => out,
            Err(e) => raise_error!(
                "ERR_VPN_PING_EXEC_FAIL",
                error = e,
                context = json_value!({
                    "target_ip": peer_ip,
                    "action": "ping_vpn_peer",
                    "hint": "Le binaire 'ping' n'a pas pu être exécuté. Vérifiez les permissions d'exécution ou la présence du binaire sur le système hôte."
                })
            ),
        };

        Ok(output.status.success())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_network_config_default() {
        let config = NetworkConfig::default();
        assert_eq!(config.name, "raise");
        assert_eq!(config.cidr, "10.42.0.0/16");
    }

    #[async_test]
    async fn test_innernet_client_creation() {
        let config = NetworkConfig::default();
        let client = InnernetClient::new(config);

        let status = client.status.read().await;
        assert!(!status.connected);
    }

    #[test]
    fn test_parse_wg_output() {
        let config = NetworkConfig::default();
        let client = InnernetClient::new(config);

        let wg_output = r#"
interface: raise0
  public key: abc123...
  private key: (hidden)
  listening port: 51820

peer: def456...
  endpoint: 192.168.1.100:51820
  allowed ips: 10.42.1.1/32
  latest handshake: 30 seconds ago
  transfer: 1.5 KiB received, 2.3 KiB sent
        "#;

        let peers = client.parse_wg_output(wg_output).unwrap();
        assert_eq!(peers.len(), 1);
        assert_eq!(peers[0].ip, "10.42.1.1");
    }
}
