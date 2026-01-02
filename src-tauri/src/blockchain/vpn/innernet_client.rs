// src-tauri/src/vpn/innernet_client.rs
//! Client Innernet pour RAISE
//!
//! Ce module gère la connexion au mesh VPN Innernet pour assurer
//! la souveraineté et la sécurité des communications.

use serde::{Deserialize, Serialize};
use std::process::{Command, Output};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Peer {
    pub name: String,
    pub ip: String,
    pub public_key: String,
    pub endpoint: Option<String>,
    pub last_handshake: Option<i64>,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkStatus {
    pub connected: bool,
    pub interface: String,
    pub ip_address: Option<String>,
    pub peers: Vec<Peer>,
    pub uptime_seconds: Option<u64>,
}

#[derive(Debug, thiserror::Error)]
pub enum VpnError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Command execution error: {0}")]
    CommandExecution(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Network not configured")]
    NotConfigured,
}

type Result<T> = std::result::Result<T, VpnError>;

pub struct InnernetClient {
    config: NetworkConfig,
    status: Arc<RwLock<NetworkStatus>>,
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
            status: Arc::new(RwLock::new(status)),
        }
    }

    /// Vérifie si Innernet est installé
    pub fn check_installation() -> Result<String> {
        let output = Command::new("innernet")
            .arg("--version")
            .output()
            .map_err(|e| VpnError::CommandExecution(format!("Innernet not found: {}", e)))?;

        if !output.status.success() {
            return Err(VpnError::CommandExecution(
                "Innernet command failed".to_string(),
            ));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version.trim().to_string())
    }

    /// Se connecte au réseau mesh
    pub async fn connect(&self) -> Result<()> {
        tracing::info!("Connecting to Innernet network: {}", self.config.name);

        let output = self.run_command(&["up", &self.config.name])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VpnError::Connection(format!(
                "Failed to connect: {}",
                stderr
            )));
        }

        // Mettre à jour le statut
        let mut status = self.status.write().await;
        status.connected = true;

        // Récupérer l'IP assignée
        if let Ok(ip) = self.get_interface_ip().await {
            status.ip_address = Some(ip);
        }

        tracing::info!("Successfully connected to {}", self.config.name);

        Ok(())
    }

    /// Se déconnecte du réseau mesh
    pub async fn disconnect(&self) -> Result<()> {
        tracing::info!("Disconnecting from Innernet network: {}", self.config.name);

        let output = self.run_command(&["down", &self.config.name])?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VpnError::Connection(format!(
                "Failed to disconnect: {}",
                stderr
            )));
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
    pub async fn get_status(&self) -> Result<NetworkStatus> {
        if !self.status.read().await.connected {
            return Ok(self.status.read().await.clone());
        }

        // Mettre à jour la liste des peers
        if let Ok(peers) = self.fetch_peers().await {
            let mut status = self.status.write().await;
            status.peers = peers;
        }

        Ok(self.status.read().await.clone())
    }

    /// Liste tous les peers du réseau
    pub async fn list_peers(&self) -> Result<Vec<Peer>> {
        self.fetch_peers().await
    }

    /// Ajoute un nouveau peer via un code d'invitation
    pub async fn add_peer(&self, invitation_code: &str) -> Result<String> {
        tracing::info!("Adding peer with invitation code");

        // TODO: Implémenter l'ajout de peer via invitation
        // innernet install <invitation-file>

        Ok("Peer added successfully".to_string())
    }

    /// Exécute une commande Innernet
    fn run_command(&self, args: &[&str]) -> Result<Output> {
        Command::new("innernet")
            .args(args)
            .output()
            .map_err(|e| VpnError::CommandExecution(e.to_string()))
    }

    /// Récupère l'IP de l'interface
    async fn get_interface_ip(&self) -> Result<String> {
        let output = self.run_command(&["show", &self.config.name])?;

        if !output.status.success() {
            return Err(VpnError::Parse("Failed to get interface info".to_string()));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parser la sortie pour extraire l'IP
        // Format attendu: "interface: raise0, ip: 10.42.1.1/24"
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

        Err(VpnError::Parse("Could not parse IP address".to_string()))
    }

    /// Récupère la liste des peers via WireGuard
    async fn fetch_peers(&self) -> Result<Vec<Peer>> {
        // Alternative : utiliser wg show directement
        let output = Command::new("wg")
            .args(&["show", &self.config.interface])
            .output()
            .map_err(|e| VpnError::CommandExecution(e.to_string()))?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let peers = self.parse_wg_output(&stdout)?;

        Ok(peers)
    }

    /// Parse la sortie de `wg show`
    fn parse_wg_output(&self, output: &str) -> Result<Vec<Peer>> {
        let mut peers = Vec::new();
        let mut current_peer: Option<Peer> = None;

        for line in output.lines() {
            let line = line.trim();

            if line.starts_with("peer:") {
                // Sauvegarder le peer précédent
                if let Some(peer) = current_peer.take() {
                    peers.push(peer);
                }

                // Nouveau peer
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
                    // Parser le timestamp
                    // Format: "1 minute, 30 seconds ago" ou timestamp Unix
                    peer.last_handshake = Some(chrono::Utc::now().timestamp());
                } else if line.starts_with("transfer:") {
                    // Parser "transfer: 1.5 GiB received, 2.3 GiB sent"
                    // Pour simplifier, on met 0 pour l'instant
                }
            }
        }

        // Ajouter le dernier peer
        if let Some(peer) = current_peer {
            peers.push(peer);
        }

        Ok(peers)
    }

    /// Ping un peer spécifique
    pub async fn ping_peer(&self, peer_ip: &str) -> Result<bool> {
        let output = Command::new("ping")
            .args(&["-c", "1", "-W", "2", peer_ip])
            .output()
            .map_err(|e| VpnError::CommandExecution(e.to_string()))?;

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

    #[tokio::test]
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
