pub mod leann_store;
pub mod qdrant_store;

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub metadata: serde_json::Value,
    pub vectors: Option<Vec<f32>>,
}

#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn init_collection(&self, collection_name: &str, vector_size: u64) -> Result<()>;
    async fn add_documents(&self, collection_name: &str, records: Vec<MemoryRecord>) -> Result<()>;

    // Signature à 5 paramètres (hors self)
    async fn search_similarity(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
        score_threshold: f32,
        filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<MemoryRecord>>;
}

#[cfg(test)]
mod integration_tests {
    use super::qdrant_store::QdrantMemory;
    use super::{MemoryRecord, VectorStore};
    use serde_json::json;
    use std::env;
    use uuid::Uuid;

    #[tokio::test]
    #[ignore]
    async fn test_qdrant_lifecycle() {
        dotenvy::dotenv().ok();
        let port = env::var("PORT_QDRANT_GRPC").unwrap_or("6334".to_string());
        let url = format!("http://127.0.0.1:{}", port);

        let store = QdrantMemory::new(&url)
            .unwrap_or_else(|e| panic!("❌ Qdrant inaccessible sur {} : {}", url, e));

        let col = "integ_test_collection";
        store.init_collection(col, 4).await.expect("Init failed");

        let rec = MemoryRecord {
            id: Uuid::new_v4().to_string(),
            content: "Test d'intégration".into(),
            metadata: json!({"env": "test"}),
            vectors: Some(vec![1.0, 0.0, 0.0, 0.0]),
        };

        store.add_documents(col, vec![rec.clone()]).await.unwrap();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Appel avec 5 arguments
        let res = store
            .search_similarity(col, &[1.0, 0.0, 0.0, 0.0], 1, 0.0, None)
            .await
            .unwrap();
        assert!(!res.is_empty());
    }
}
