// src-tauri/src/blockchain/fabric/client.rs
//! Client Hyperledger Fabric (Implémentation pour Tonic 0.14.3).

use crate::utils::{io::Path, prelude::*, Duration}; // 🎯 Ajout du prelude RAISE (RaiseResult, json!)
                                                    // Ces imports sont maintenant disponibles grâce à la feature "tls-ring"
use tonic::transport::{Certificate, Channel, ClientTlsConfig, Identity};

use super::config::ConnectionProfile;

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

    pub async fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> RaiseResult<Self> {
        let path_str = path.as_ref().display().to_string();

        let content = match tokio::fs::read_to_string(&path).await {
            Ok(c) => c,
            Err(e) => {
                raise_error!(
                    "ERR_FABRIC_PROFILE_READ",
                    error = e, // On passe l'erreur brute, la macro s'occupe de la conversion
                    context = json!({
                        "file_path": path_str,
                        "action": "load_fabric_connection_profile",
                        "hint": "Vérifiez que le fichier existe et que les droits de lecture sont accordés."
                    })
                )
            }
        };

        let config: ConnectionProfile = match serde_yaml::from_str(&content) {
            Ok(profile) => profile,
            Err(e) => {
                raise_error!(
                    "ERR_FABRIC_PROFILE_PARSE",
                    error = e, // On injecte l'erreur Serde directement
                    context = json!({
                        "file_path": path_str,
                        "action": "parse_fabric_yaml_profile",
                        "hint": "Le fichier YAML est mal formé ou des champs obligatoires sont manquants (ex: 'organizations', 'peers', 'certificateAuthorities')."
                    })
                )
            }
        };

        Ok(Self::from_config(config))
    }

    pub async fn with_identity<P: AsRef<Path>>(
        mut self,
        cert_path: P,
        key_path: P,
    ) -> RaiseResult<Self> {
        let cert_str = cert_path.as_ref().display().to_string();
        let key_str = key_path.as_ref().display().to_string();

        let cert = match tokio::fs::read(&cert_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                raise_error!(
                    "ERR_FABRIC_CRYPTO_CERT_READ",
                    error = e, // On passe l'erreur I/O brute
                    context = json!({
                        "cert_path": cert_str,
                        "action": "load_fabric_certificate",
                        "hint": "Le fichier de certificat (.pem) est introuvable ou illisible. Vérifiez le dossier crypto-config."
                    })
                )
            }
        };

        let key = match tokio::fs::read(&key_path).await {
            Ok(bytes) => bytes,
            Err(e) => {
                raise_error!(
                    "ERR_FABRIC_CRYPTO_KEY_READ",
                    error = e, // On préserve l'erreur IO originale
                    context = json!({
                        "key_path": key_str,
                        "action": "load_fabric_private_key",
                        "hint": "La clé privée est introuvable. Sous Fabric, le nom du fichier finit souvent par '_sk'. Vérifiez le dossier keystore."
                    })
                )
            }
        };

        self.identity = Some(Identity::from_pem(cert, key));
        Ok(self)
    }

    pub async fn connect(&mut self, peer_name: &str) -> RaiseResult<()> {
        let peer_config = match self.config.peers.get(peer_name) {
            Some(config) => config,
            None => raise_error!(
                "ERR_FABRIC_CONFIG_PEER_NOT_FOUND",
                context = json!({
                    "peer_name": peer_name,
                    "action": "connect_to_fabric_peer",
                    "hint": "Le nœud demandé n'existe pas dans la configuration YAML. Vérifiez l'orthographe dans la section 'peers'."
                })
            ),
        };

        // 1. Création de l'endpoint
        let mut endpoint = match Channel::from_shared(peer_config.url.clone()) {
            Ok(ch) => ch,
            Err(e) => raise_error!(
                "ERR_FABRIC_GRPC_ENDPOINT_INVALID",
                error = e,
                context = json!({
                    "peer_name": peer_name,
                    "url": peer_config.url,
                    "action": "create_grpc_channel",
                    "hint": "L'URL fournie n'est pas un endpoint gRPC valide. Vérifiez le format (ex: http://127.0.0.1:7051)."
                })
            ),
        };

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
            endpoint = match endpoint.tls_config(tls) {
                Ok(ep) => ep,
                Err(e) => raise_error!(
                    "ERR_FABRIC_TLS_CONFIG_FAIL",
                    error = e,
                    context = json!({
                        "peer_name": peer_name,
                        "action": "configure_tls_connection",
                        "hint": "Échec de l'application de la configuration TLS. Vérifiez que le certificat CA est valide et que le 'ssl-target-name-override' correspond au domaine du Peer."
                    })
                ),
            };
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
    ) -> RaiseResult<String> {
        if self.channel.is_none() {
            crate::raise_error!(
                "ERR_FABRIC_GRPC_NOT_CONNECTED",
                error = "Le canal gRPC n'est pas initialisé.",
                context = json!({
                    "chaincode": chaincode,
                    "function": func,
                    "action": "submit_transaction",
                    "hint": "Vous devez appeler 'connect()' avant de soumettre une transaction au réseau."
                })
            );
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

    pub async fn query_transaction(&self, func: &str, args: Vec<Vec<u8>>) -> RaiseResult<Vec<u8>> {
        if self.channel.is_none() {
            crate::raise_error!(
                "ERR_FABRIC_GRPC_NOT_CONNECTED",
                error = "Le canal gRPC n'est pas initialisé.",
                context = json!({
                    "function": func,
                    "action": "query_transaction",
                    "hint": "Vous devez appeler 'connect()' avant d'effectuer une requête."
                })
            );
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
