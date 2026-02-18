// src-tauri/src/blockchain/fabric/client.rs
//! Client Hyperledger Fabric (Implémentation pour Tonic 0.14.3).

use crate::utils::{io::Path, Duration};
// Ces imports sont maintenant disponibles grâce à la feature "tls-ring"
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};

use super::config::ConnectionProfile;
use crate::blockchain::error::FabricError;

type Result<T> = std::result::Result<T, FabricError>;

#[derive(Debug, Clone)]
pub struct FabricClient {
    config: ConnectionProfile,
    channel: Option<Channel>,
    identity: Option<Identity>,
}

impl FabricClient {
    pub fn from_config(config: ConnectionProfile) -> Self {
        Self {
            config,
            channel: None,
            identity: None,
        }
    }

    pub async fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = tokio::fs::read_to_string(path)
            .await
            .map_err(|e| FabricError::ProfileParse(format!("File read error: {}", e)))?;

        let config: ConnectionProfile = serde_yaml::from_str(&content)
            .map_err(|e| FabricError::ProfileParse(format!("YAML error: {}", e)))?;

        Ok(Self::from_config(config))
    }

    pub async fn with_identity<P: AsRef<Path>>(
        mut self,
        cert_path: P,
        key_path: P,
    ) -> Result<Self> {
        let cert = tokio::fs::read(cert_path)
            .await
            .map_err(|e| FabricError::Crypto(format!("Cert read failed: {}", e)))?;

        let key = tokio::fs::read(key_path)
            .await
            .map_err(|e| FabricError::Crypto(format!("Key read failed: {}", e)))?;

        self.identity = Some(Identity::from_pem(cert, key));
        Ok(self)
    }

    pub async fn connect(&mut self, peer_name: &str) -> Result<()> {
        let peer_config = self.config.peers.get(peer_name).ok_or_else(|| {
            FabricError::Config(format!("Peer '{}' not found in profile", peer_name))
        })?;

        // 1. Création de l'endpoint
        let mut endpoint = Channel::from_shared(peer_config.url.clone())
            .map_err(|e| FabricError::Config(format!("Invalid Peer URL: {}", e)))?;

        // 2. Configuration TLS si présente
        if let Some(ref tls_conf) = peer_config.tls_ca_certs.pem {
            let ca = Certificate::from_pem(tls_conf);

            let mut tls = ClientTlsConfig::new()
                .ca_certificate(ca)
                .domain_name(peer_name);

            if let Some(ref id) = self.identity {
                tls = tls.identity(id.clone());
            }

            // Cette méthode est disponible car tls-ring active le support TLS
            endpoint = endpoint
                .tls_config(tls)
                .map_err(|e| FabricError::GrpcConnection(format!("TLS config failed: {}", e)))?;
        }

        // 3. Connexion (Lazy)
        let channel = endpoint.timeout(Duration::from_secs(10)).connect_lazy();

        self.channel = Some(channel);
        tracing::info!("🔗 [Fabric] Canal gRPC configuré pour {}", peer_name);

        Ok(())
    }

    pub async fn submit_transaction(
        &self,
        chaincode: &str,
        func: &str,
        args: Vec<String>,
    ) -> Result<String> {
        if self.channel.is_none() {
            return Err(FabricError::GrpcConnection(
                "Channel not initialized. Call connect() first.".into(),
            ));
        }

        let log_msg = format!(
            "Would invoke chaincode '{}' function '{}' with args {:?}",
            chaincode, func, args
        );
        tracing::debug!("{}", log_msg);

        // Simulation réseau
        tokio::time::sleep(Duration::from_millis(100)).await;

        Ok(format!("TX_ID_MOCK_12345 ({})", log_msg))
    }

    pub async fn query_transaction(&self, func: &str, args: Vec<Vec<u8>>) -> Result<Vec<u8>> {
        if self.channel.is_none() {
            return Err(FabricError::GrpcConnection("No channel".into()));
        }
        tracing::debug!("Querying '{}' with {} args", func, args.len());
        Ok(b"Query Result Mock".to_vec())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fabric_client_lifecycle() {
        let mut config = ConnectionProfile {
            name: "test".into(),
            version: "1.0".into(),
            client: super::super::config::ClientConfig {
                organization: "Org1".into(),
                connection: None,
            },
            organizations: std::collections::HashMap::new(),
            peers: std::collections::HashMap::new(),
            certificate_authorities: std::collections::HashMap::new(),
        };

        config.peers.insert(
            "peer0".into(),
            super::super::config::PeerConfig {
                url: "http://localhost:50051".into(),
                tls_ca_certs: super::super::config::TlsConfig {
                    pem: None,
                    path: None,
                },
                grpc_options: None,
            },
        );

        let mut client = FabricClient::from_config(config);

        // Test connect_lazy
        let res = client.connect("peer0").await;
        assert!(res.is_ok());

        // Test transaction mock
        let tx = client.submit_transaction("cc", "fn", vec![]).await;
        assert!(tx.is_ok());
    }
}
