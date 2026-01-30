// src-tauri/src/blockchain/fabric/config.rs
//! Modèle de données pour le Connection Profile d'Hyperledger Fabric.
//!
//! Ce fichier permet de parser le YAML standard (Common Connection Profile - CCP)
//! utilisé par les SDKs Fabric pour identifier les pairs, les CAs et les MSPs.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Racine du Connection Profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionProfile {
    pub name: String,
    pub version: String,
    pub client: ClientConfig,
    pub organizations: HashMap<String, OrganizationConfig>,
    pub peers: HashMap<String, PeerConfig>,
    #[serde(rename = "certificateAuthorities")]
    pub certificate_authorities: HashMap<String, CaConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub organization: String,
    #[serde(rename = "connection")]
    pub connection: Option<ConnectionDefaults>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionDefaults {
    pub timeout: Option<ConnectionTimeout>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConnectionTimeout {
    pub peer: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrganizationConfig {
    pub mspid: String,
    pub peers: Vec<String>,
    #[serde(rename = "certificateAuthorities")]
    pub certificate_authorities: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerConfig {
    pub url: String,
    #[serde(rename = "tlsCACerts")]
    pub tls_ca_certs: TlsConfig,
    #[serde(rename = "grpcOptions")]
    pub grpc_options: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaConfig {
    pub url: String,
    #[serde(rename = "caName")]
    pub ca_name: Option<String>,
    #[serde(rename = "tlsCACerts")]
    pub tls_ca_certs: Option<TlsConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TlsConfig {
    pub pem: Option<String>,
    pub path: Option<String>,
}

// --- Tests Unitaires ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_connection_profile() {
        // Simulation d'un fichier connection-profile.yaml standard
        let yaml_data = r#"
name: "raise-network"
version: "1.0.0"
client:
  organization: "Org1"
  connection:
    timeout:
      peer:
        endorser: "300"
organizations:
  Org1:
    mspid: "Org1MSP"
    peers:
      - "peer0.org1.example.com"
    certificateAuthorities:
      - "ca.org1.example.com"
peers:
  "peer0.org1.example.com":
    url: "grpcs://localhost:7051"
    tlsCACerts:
      pem: "-----BEGIN CERTIFICATE-----FAKE_CERT-----END CERTIFICATE-----"
certificateAuthorities:
  "ca.org1.example.com":
    url: "https://localhost:7054"
    caName: "ca-org1"
"#;

        // Tentative de désérialisation
        let config: ConnectionProfile =
            serde_yaml::from_str(yaml_data).expect("Échec du parsing YAML du Connection Profile");

        // Vérifications
        assert_eq!(config.name, "raise-network");
        assert_eq!(config.client.organization, "Org1");
        assert_eq!(config.organizations["Org1"].mspid, "Org1MSP");
        assert_eq!(
            config.peers["peer0.org1.example.com"].url,
            "grpcs://localhost:7051"
        );

        // Vérification TLS
        let peer_tls = &config.peers["peer0.org1.example.com"].tls_ca_certs;
        assert!(peer_tls.pem.as_ref().unwrap().contains("FAKE_CERT"));
    }
}
