// FICHIER : src-tauri/src/ai/memory/mod.rs

use crate::utils::prelude::*;

pub mod candle_store;

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub metadata: JsonValue,
    pub vectors: Option<Vec<f32>>,
}

#[async_interface]
pub trait VectorStore: Send + Sync {
    async fn init_collection(&self, collection_name: &str, vector_size: u64) -> RaiseResult<()>;
    async fn add_documents(
        &self,
        collection_name: &str,
        records: Vec<MemoryRecord>,
    ) -> RaiseResult<()>;

    // Signature à 5 paramètres (hors self)
    async fn search_similarity(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
        score_threshold: f32,
        filter: Option<UnorderedMap<String, String>>,
    ) -> RaiseResult<Vec<MemoryRecord>>;
}

#[cfg(test)]
mod integration_tests {
    use super::{MemoryRecord, VectorStore};
    use crate::ai::memory::candle_store::CandleLocalStore;
    use crate::utils::prelude::*;
    use candle_core::Device;

    #[async_test]
    async fn test_candle_lifecycle() {
        // ✅ 1. Création d'un espace isolé et 100% local (plus besoin d'URL ou de port !)
        let dir = tempdir().unwrap();
        let device = Device::Cpu;
        let store_dir = dir.path().join("vector_store");

        let store = CandleLocalStore::new(&store_dir, &device);

        let col = "integ_test_collection";
        store.init_collection(col, 4).await.expect("Init failed");

        let rec = MemoryRecord {
            id: UniqueId::new_v4().to_string(),
            content: "Test d'intégration natif".into(),
            metadata: json_value!({"env": "test"}),
            vectors: Some(vec![1.0, 0.0, 0.0, 0.0]),
        };

        // ✅ 2. Ajout du document et sauvegarde explicite sur le disque
        store.add_documents(col, vec![rec.clone()]).await.unwrap();
        store.save().await.expect("Échec de la persistance locale");

        // ✅ 3. Recherche avec les 5 arguments
        let res = store
            .search_similarity(col, &[1.0, 0.0, 0.0, 0.0], 1, 0.0, None)
            .await
            .unwrap();

        assert!(!res.is_empty(), "La recherche doit remonter le document");
        assert_eq!(res[0].content, "Test d'intégration natif");
    }
}
