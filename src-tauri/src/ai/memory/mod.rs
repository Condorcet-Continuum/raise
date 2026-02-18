use crate::utils::{async_trait, data, prelude::*, HashMap};

pub mod leann_store;
pub mod qdrant_store;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub metadata: data::Value,
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
    use crate::utils::{config::AppConfig, prelude::*, sleep, Duration, Once, Uuid};

    static INIT_TEST: Once = Once::new();

    #[tokio::test]
    #[ignore]
    async fn test_qdrant_lifecycle() {
        // ✅ 1. bonjour SSOT !
        INIT_TEST.call_once(|| {
            let _ = AppConfig::init();
        });

        // ✅ 2. On récupère le port depuis la configuration centralisée
        let config = AppConfig::get();
        let port = config
            .services
            .get("qdrant_grpc")
            .map(|s| s.port)
            .unwrap_or(6334);

        let url = format!("http://127.0.0.1:{}", port);

        let store = QdrantMemory::new(&url)
            .unwrap_or_else(|e| panic!("❌ Qdrant inaccessible sur {} : {}", url, e));

        let col = "integ_test_collection";
        store.init_collection(col, 4).await.expect("Init failed");

        let rec = MemoryRecord {
            id: Uuid::new_v4().to_string(), // ✅ L'import Uuid fonctionne
            content: "Test d'intégration".into(),
            metadata: json!({"env": "test"}), // ✅ Correction de la typo : json! au lieu de data:json!
            vectors: Some(vec![1.0, 0.0, 0.0, 0.0]),
        };

        store.add_documents(col, vec![rec.clone()]).await.unwrap();
        sleep(Duration::from_millis(500)).await;

        // Appel avec 5 arguments
        let res = store
            .search_similarity(col, &[1.0, 0.0, 0.0, 0.0], 1, 0.0, None)
            .await
            .unwrap();
        assert!(!res.is_empty());
    }
}
