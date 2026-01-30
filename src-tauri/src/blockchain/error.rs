// FICHIER : src-tauri/src/blockchain/error.rs

use serde::Serialize;
use thiserror::Error;

/// Enum principal regroupant toutes les erreurs du module Blockchain.
/// Visible par tous les modules (AI, Traceability, etc.).
#[derive(Debug, Error)]
pub enum BlockchainError {
    #[error("VPN Error: {0}")]
    Vpn(#[from] VpnError),

    #[error("Fabric Error: {0}")]
    Fabric(#[from] FabricError),

    #[error("Configuration Error: {0}")]
    Config(String),

    #[error("IO Error: {0}")]
    Io(String),

    #[error("Unknown Error: {0}")]
    Unknown(String),
}

// Implémentation manuelle de Serialize pour renvoyer des messages propres au Frontend/Logs
impl Serialize for BlockchainError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

// Conversion pour std::io::Error (fréquent dans les ops réseau/fichiers)
impl From<std::io::Error> for BlockchainError {
    fn from(err: std::io::Error) -> Self {
        BlockchainError::Io(err.to_string())
    }
}

// --- Sous-erreurs VPN (Innernet / WireGuard) ---

#[derive(Debug, Error, Serialize)]
pub enum VpnError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Command execution error: {0}")]
    CommandExecution(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Network not configured")]
    NotConfigured,

    #[error("Timeout error: {0}")]
    Timeout(String),
}

// --- Sous-erreurs Fabric (Hyperledger) ---

#[derive(Debug, Error, Serialize)]
pub enum FabricError {
    #[error("Profile parsing error: {0}")]
    ProfileParse(String),

    #[error("gRPC Connection error: {0}")]
    GrpcConnection(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Crypto/MSP error: {0}")]
    Crypto(String),

    #[error("Chaincode error: {0}")]
    Chaincode(String),

    #[error("Configuration error: {0}")] // <--- AJOUTEZ CECI
    Config(String),
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vpn_error_serialization() {
        let err = VpnError::Connection("Timeout handshake".into());
        let wrapper: BlockchainError = err.into();

        // Vérifie que l'erreur est bien convertie en JSON String pour le frontend
        let json = serde_json::to_string(&wrapper).unwrap();
        assert_eq!(json, "\"VPN Error: Connection error: Timeout handshake\"");
    }

    #[test]
    fn test_fabric_error_structure() {
        let err = FabricError::GrpcConnection("Peer unavailable".into());
        let wrapper: BlockchainError = err.into();

        match wrapper {
            BlockchainError::Fabric(e) => {
                assert!(matches!(e, FabricError::GrpcConnection(_)));
            }
            _ => panic!("Mauvais mapping d'erreur Fabric"),
        }
    }

    #[test]
    fn test_io_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "config.yaml missing");
        let wrapper: BlockchainError = io_err.into();

        assert_eq!(wrapper.to_string(), "IO Error: config.yaml missing");
    }
}
