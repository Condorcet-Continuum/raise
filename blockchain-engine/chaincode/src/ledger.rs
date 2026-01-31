// blockchain-engine/chaincode/src/ledger.rs
use async_trait::async_trait;
use serde::{de::DeserializeOwned, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex}; // Import explicite de la macro

/// Le Trait qui définit ce qu'est un Ledger (Abstraction).
#[async_trait] // La macro rend le trait "Dyn Compatible" (Object Safe)
pub trait LedgerService: Send + Sync {
    async fn get_state_bytes(&self, id: &str) -> Result<Option<Vec<u8>>, String>;
    async fn put_state_bytes(&self, id: &str, value: Vec<u8>) -> Result<(), String>;
    async fn exists(&self, id: &str) -> Result<bool, String>;
}

/// Le Contexte passé au contrat pour chaque transaction.
pub struct LedgerContext {
    service: Arc<dyn LedgerService>,
}

impl LedgerContext {
    pub fn new(service: Arc<dyn LedgerService>) -> Self {
        Self { service }
    }

    pub async fn get_state<T: DeserializeOwned>(&self, id: &str) -> Result<Option<T>, String> {
        let bytes_opt = self.service.get_state_bytes(id).await?;
        match bytes_opt {
            Some(bytes) => {
                let item: T = serde_json::from_slice(&bytes)
                    .map_err(|e| format!("Ledger JSON Deserialization error for {}: {}", id, e))?;
                Ok(Some(item))
            }
            None => Ok(None),
        }
    }

    pub async fn put_state<T: Serialize>(&self, id: &str, value: &T) -> Result<(), String> {
        let bytes = serde_json::to_vec(value)
            .map_err(|e| format!("Ledger JSON Serialization error for {}: {}", id, e))?;

        self.service.put_state_bytes(id, bytes).await
    }

    pub async fn exists(&self, id: &str) -> Result<bool, String> {
        self.service.exists(id).await
    }
}

// ==================================================================================
// IMPLEMENTATION MOCK
// ==================================================================================

#[derive(Clone, Default)]
pub struct MockLedger {
    store: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl MockLedger {
    pub fn new() -> Self {
        Self {
            store: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl LedgerService for MockLedger {
    async fn get_state_bytes(&self, id: &str) -> Result<Option<Vec<u8>>, String> {
        let store = self.store.lock().unwrap();
        Ok(store.get(id).cloned())
    }

    async fn put_state_bytes(&self, id: &str, value: Vec<u8>) -> Result<(), String> {
        let mut store = self.store.lock().unwrap();
        store.insert(id.to_string(), value);
        Ok(())
    }

    async fn exists(&self, id: &str) -> Result<bool, String> {
        let store = self.store.lock().unwrap();
        Ok(store.contains_key(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestAsset {
        id: String,
        value: i32,
    }

    #[tokio::test]
    async fn test_ledger_operations() {
        let mock_service = Arc::new(MockLedger::new());
        let ctx = LedgerContext::new(mock_service);

        let asset_id = "asset_01";
        let asset = TestAsset {
            id: asset_id.to_string(),
            value: 42,
        };

        ctx.put_state(asset_id, &asset).await.expect("Put failed");

        let exists = ctx.exists(asset_id).await.expect("Exists failed");
        assert!(exists);

        let retrieved: Option<TestAsset> = ctx.get_state(asset_id).await.expect("Get failed");
        assert_eq!(retrieved, Some(asset));
    }
}
