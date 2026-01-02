// src-tauri/src/fabric/client.rs
//! Client Hyperledger Fabric pour RAISE
//!
//! Ce module implémente un client léger pour interagir avec un réseau
//! Hyperledger Fabric via gRPC, optimisé pour l'intégration Tauri.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FabricConfig {
    pub endpoint: String,
    pub msp_id: String,
    pub channel_name: String,
    pub chaincode_name: String,
    pub tls_enabled: bool,
}

impl Default for FabricConfig {
    fn default() -> Self {
        Self {
            endpoint: "grpc://localhost:7051".to_string(),
            msp_id: "RAISEMSP".to_string(),
            channel_name: "raise-channel".to_string(),
            chaincode_name: "arcadia-chaincode".to_string(),
            tls_enabled: false,
        }
    }
}

pub struct FabricClient {
    config: FabricConfig,
    identity: Arc<RwLock<Option<Identity>>>,
}

#[derive(Debug, Clone)]
pub struct Identity {
    pub msp_id: String,
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TransactionResult {
    pub transaction_id: String,
    pub status: String,
    pub payload: Vec<u8>,
    pub timestamp: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum FabricError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Transaction error: {0}")]
    Transaction(String),

    #[error("Identity error: {0}")]
    Identity(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

type Result<T> = std::result::Result<T, FabricError>;

impl FabricClient {
    /// Crée une nouvelle instance du client Fabric
    pub fn new(config: FabricConfig) -> Self {
        Self {
            config,
            identity: Arc::new(RwLock::new(None)),
        }
    }

    /// Charge l'identité MSP depuis les fichiers de certificat
    pub async fn load_identity(&self, cert_path: &str, key_path: &str) -> Result<()> {
        use tokio::fs;

        let certificate = fs::read(cert_path)
            .await
            .map_err(|e| FabricError::Identity(format!("Failed to read cert: {}", e)))?;

        let private_key = fs::read(key_path)
            .await
            .map_err(|e| FabricError::Identity(format!("Failed to read key: {}", e)))?;

        let identity = Identity {
            msp_id: self.config.msp_id.clone(),
            certificate,
            private_key,
        };

        *self.identity.write().await = Some(identity);

        Ok(())
    }

    /// Soumet une transaction au réseau Fabric
    pub async fn submit_transaction(
        &self,
        function: &str,
        args: Vec<Vec<u8>>,
    ) -> Result<TransactionResult> {
        let identity = self.identity.read().await;
        let identity = identity
            .as_ref()
            .ok_or_else(|| FabricError::Identity("No identity loaded".to_string()))?;

        // TODO: Implémenter l'appel gRPC vers le peer Fabric
        // Pour l'instant, on simule une réponse

        let tx_id = uuid::Uuid::new_v4().to_string();
        let timestamp = chrono::Utc::now().timestamp();

        tracing::info!(
            "Submitting transaction to {}/{}: {} with {} args",
            self.config.channel_name,
            self.config.chaincode_name,
            function,
            args.len()
        );

        Ok(TransactionResult {
            transaction_id: tx_id,
            status: "VALID".to_string(),
            payload: Vec::new(),
            timestamp,
        })
    }

    /// Effectue une requête en lecture seule
    pub async fn query_transaction(&self, function: &str, args: Vec<Vec<u8>>) -> Result<Vec<u8>> {
        let identity = self.identity.read().await;
        let _identity = identity
            .as_ref()
            .ok_or_else(|| FabricError::Identity("No identity loaded".to_string()))?;

        // TODO: Implémenter l'appel gRPC query

        tracing::info!(
            "Querying {}/{}: {}",
            self.config.channel_name,
            self.config.chaincode_name,
            function
        );

        Ok(Vec::new())
    }

    /// Récupère l'historique d'une clé
    pub async fn get_history(&self, key: &str) -> Result<Vec<TransactionResult>> {
        // TODO: Implémenter GetHistoryForKey

        tracing::info!("Getting history for key: {}", key);

        Ok(Vec::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fabric_client_creation() {
        let config = FabricConfig::default();
        let client = FabricClient::new(config);

        assert!(client.identity.read().await.is_none());
    }

    #[tokio::test]
    async fn test_transaction_submission() {
        let config = FabricConfig::default();
        let client = FabricClient::new(config);

        // Note: Ce test échouera sans identité chargée
        let result = client
            .submit_transaction("RecordDecision", vec![b"test".to_vec()])
            .await;

        assert!(result.is_err());
    }
}
