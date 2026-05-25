// src-tauri/src/blockchain/vpn/innernet_client.rs
//! Client Innernet RAISE : Orchestration du maillage VPN WireGuard sécurisé.

use crate::utils::prelude::*;

/// Configuration du segment réseau mesh.
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

/// Représentation d'un agent (pair) détecté sur le segment VPN.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct Peer {
    pub name: String,
    pub ip: String,
    pub public_key: String,
    pub endpoint: Option<String>,
    pub last_handshake: Option<i64>,
    pub transfer_rx: u64,
    pub transfer_tx: u64,
}

/// État de santé et télémétrie du réseau[cite: 9].
#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct NetworkStatus {
    pub connected: bool,
    pub interface: String,
    pub ip_address: Option<String>,
    pub peers: Vec<Peer>,
    pub uptime_seconds: Option<u64>,
}

/// Client d'orchestration pour la CLI Innernet.
#[derive(Clone)]
pub struct InnernetClient {
    config: NetworkConfig,
    status: SharedRef<AsyncRwLock<NetworkStatus>>,
}

impl InnernetClient {
    /// Initialise une nouvelle instance du client avec état atomique partagé[cite: 14].
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

    /// Vérifie la présence et la version du binaire innernet sur l'hôte[cite: 14].
    pub fn check_installation() -> RaiseResult<String> {
        let cmd = ProcessCommand::new("innernet").arg("--version").output();

        match cmd {
            Ok(output) if output.status.success() => {
                Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
            }
            Ok(output) => raise_error!(
                "ERR_VPN_BINARY_INCORRECT",
                context = json_value!({ "exit_code": output.status.code() })
            ),
            Err(e) => raise_error!("ERR_VPN_BINARY_MISSING", error = e),
        }
    }

    /// Active l'interface VPN et met à jour l'IP locale[cite: 9, 13].
    pub async fn connect(&self) -> RaiseResult<()> {
        user_info!(
            "INF_VPN_CONNECTING",
            json_value!({ "network": self.config.name })
        );

        self.run_command(["up", &self.config.name]).await?;

        let ip = self.get_interface_ip_async().await?;
        let mut status = self.status.write().await;
        status.connected = true;
        status.ip_address = Some(ip);

        user_success!("INF_VPN_CONNECTED");
        Ok(())
    }

    /// Désactive l'interface et purge l'état local[cite: 9].
    pub async fn disconnect(&self) -> RaiseResult<()> {
        self.run_command(["down", &self.config.name]).await?;

        let mut status = self.status.write().await;
        status.connected = false;
        status.ip_address = None;
        status.peers.clear();
        status.uptime_seconds = None;

        Ok(())
    }

    /// Intègre un nouveau pair via une invitation sécurisée (Écriture Atomique)[cite: 11].
    pub async fn add_peer(&self, invitation_data: &str) -> RaiseResult<String> {
        // Utilisation d'un identifiant unique pour éviter les collisions en mode multi-instance[cite: 14].
        let temp_name = format!("invite_{}.json", UniqueId::new_v4());
        let temp_path = fs::PathBuf::from("/tmp").join(temp_name);

        // Persistance sécurisée via la façade fs (RUST FIRST)[cite: 11].
        fs::write_atomic_sync(&temp_path, invitation_data.as_bytes())?;

        // Correction de durée de vie : On fige la String du chemin avant l'emprunt.
        let path_str = temp_path.to_string_lossy();

        let args = [
            "accept-invitation",
            &self.config.name,
            "--args",
            path_str.as_ref(),
        ];

        let result = self.run_command(args).await;

        // Nettoyage systématique du secret après traitement[cite: 11].
        let _ = fs::remove_file_sync(&temp_path);

        match result {
            Ok(_) => Ok("PEER_ACCEPTED".into()),
            Err(e) => Err(e),
        }
    }

    /// Exécuteur générique de commandes système avec gestion d'erreurs RAISE[cite: 12, 14].
    async fn run_command<I, S>(&self, args: I) -> RaiseResult<ProcessOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<SystemStr>,
    {
        let cmd = AsyncCommand::new("innernet").args(args).output().await;

        match cmd {
            Ok(output) if output.status.success() => Ok(output),
            Ok(output) => raise_error!(
                "ERR_VPN_COMMAND_FAILED",
                context = json_value!({ "stderr": String::from_utf8_lossy(&output.stderr).trim() })
            ),
            Err(e) => raise_error!("ERR_VPN_EXECUTION", error = e),
        }
    }

    /// Extrait l'IP de l'interface via un appel système show[cite: 9].
    async fn get_interface_ip_async(&self) -> RaiseResult<String> {
        let output = self.run_command(["show", &self.config.name]).await?;
        let stdout = String::from_utf8_lossy(&output.stdout);

        self.parse_ip_logic(&stdout)
    }

    /// Logique de parsing robuste pour l'extraction d'IP.
    fn parse_ip_logic(&self, stdout: &str) -> RaiseResult<String> {
        for line in stdout.lines() {
            if line.contains("ip:") {
                let parts: Vec<&str> = line.split("ip:").collect();
                if let Some(raw_val) = parts.get(1) {
                    let clean_ip = raw_val.trim().split('/').next().unwrap_or("");
                    if !clean_ip.is_empty() {
                        return Ok(clean_ip.to_string());
                    }
                }
            }
        }
        raise_error!("ERR_VPN_IP_NOT_FOUND")
    }

    /// Analyse la topologie WireGuard et convertit la sortie brute en modèles Peer[cite: 9].
    pub fn parse_wg_topology(&self, output: &str) -> Vec<Peer> {
        let mut peers = Vec::new();
        let mut current_peer: Option<Peer> = None;

        for line in output.lines() {
            let l = line.trim();

            if l.starts_with("peer:") {
                if let Some(p) = current_peer.take() {
                    peers.push(p);
                }
                current_peer = Some(Peer {
                    name: "unknown-peer".into(),
                    ip: "0.0.0.0".into(),
                    public_key: l.split_whitespace().nth(1).unwrap_or("").into(),
                    endpoint: None,
                    last_handshake: None,
                    transfer_rx: 0,
                    transfer_tx: 0,
                });
            } else if let Some(ref mut p) = current_peer {
                if l.starts_with("endpoint:") {
                    p.endpoint = l.split_whitespace().nth(1).map(String::from);
                } else if l.starts_with("allowed ips:") {
                    p.ip = l
                        .split(':')
                        .nth(1)
                        .and_then(|val| val.split(',').next())
                        .map(|ip| ip.trim().split('/').next().unwrap_or("0.0.0.0"))
                        .unwrap_or("0.0.0.0")
                        .into();
                } else if l.starts_with("latest handshake:") {
                    p.last_handshake = Some(UtcClock::now().timestamp());
                }
            }
        }
        if let Some(p) = current_peer {
            peers.push(p);
        }
        peers
    }
}

// =========================================================================
// TESTS DE CONFORMITÉ ET DE ROBUSTESSE
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Test 1 : Vérifie la validité des configurations par défaut[cite: 9].
    #[test]
    fn test_conformity_network_defaults() {
        let config = NetworkConfig::default();
        assert_eq!(config.name, "raise");
        assert_eq!(config.cidr, "10.42.0.0/16");
        assert!(config.interface.contains("raise"));
    }

    /// Test 2 : Analyse de la résilience du parser d'IP face à divers formats.
    #[test]
    fn test_conformity_ip_parsing() {
        let client = InnernetClient::new(NetworkConfig::default());

        // Format standard
        assert_eq!(
            client.parse_ip_logic("  ip: 10.42.0.5/32").unwrap(),
            "10.42.0.5"
        );
        // Format compact
        assert_eq!(
            client.parse_ip_logic("ip:192.168.1.1").unwrap(),
            "192.168.1.1"
        );
        // Format invalide
        assert!(client.parse_ip_logic("invalid_line: no_ip").is_err());
    }

    /// Test 3 : Simulation d'une topologie complexe (Multi-Peers)[cite: 9].
    #[test]
    fn test_conformity_wg_topology_parsing() {
        let client = InnernetClient::new(NetworkConfig::default());
        let dump = r#"
peer: pubkey_alpha
  endpoint: 1.2.3.4:51820
  allowed ips: 10.42.0.2/32

peer: pubkey_beta
  allowed ips: 10.42.0.3/32, 10.42.1.0/24
        "#;

        let peers = client.parse_wg_topology(dump);
        assert_eq!(peers.len(), 2);
        assert_eq!(peers[0].public_key, "pubkey_alpha");
        assert_eq!(peers[0].ip, "10.42.0.2");
        assert_eq!(peers[1].ip, "10.42.0.3"); // Doit extraire la première IP du range
    }

    /// Test 4 : Vérification de l'intégrité de l'état initial[cite: 14].
    #[async_test]
    async fn test_conformity_initial_state() {
        let client = InnernetClient::new(NetworkConfig::default());
        let status = client.status.read().await;

        assert_eq!(status.connected, false);
        assert_eq!(status.interface, "raise0");
        assert!(status.ip_address.is_none());
        assert!(status.uptime_seconds.is_none());
    }

    /// Test 5 : Robustesse face aux entrées corrompues.
    #[test]
    fn test_conformity_garbage_input_resilience() {
        let client = InnernetClient::new(NetworkConfig::default());
        let peers = client.parse_wg_topology("données aléatoires corrompues");
        assert!(peers.is_empty());
    }
}
