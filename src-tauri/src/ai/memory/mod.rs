// FICHIER : src-tauri/src/ai/memory/mod.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Nouvel import

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
    // 🎯 Ajout du manager en paramètre pour toutes les opérations
    async fn init_collection(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        vector_size: u64,
    ) -> RaiseResult<()>;

    async fn add_documents(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        records: Vec<MemoryRecord>,
    ) -> RaiseResult<()>;

    async fn search_similarity(
        &self,
        manager: &CollectionsManager<'_>,
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
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::prelude::*;
    use crate::utils::testing::AgentDbSandbox;
    use candle_core::Device;

    #[async_test]
    async fn test_candle_lifecycle() {
        // 🎯 On utilise désormais la Sandbox complète (Graphe + Moteur)
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        AgentDbSandbox::mock_db(&manager).await.unwrap();

        let device = Device::Cpu;
        let store_dir = sandbox.domain_root.join("vector_store");

        let store = CandleLocalStore::new(&store_dir, &device);

        let col = "integ_test_collection";
        // Création physique de la collection JSON-DB pour le test
        manager
            .create_collection(
                col,
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        store
            .init_collection(&manager, col, 4)
            .await
            .expect("Init failed");

        let rec = MemoryRecord {
            id: UniqueId::new_v4().to_string(),
            content: "Test d'intégration natif".into(),
            metadata: json_value!({"env": "test"}),
            vectors: Some(vec![1.0, 0.0, 0.0, 0.0]),
        };

        // 🎯 On passe le manager
        store
            .add_documents(&manager, col, vec![rec.clone()])
            .await
            .unwrap();
        store.save().await.expect("Échec de la persistance locale");

        // 🎯 On passe le manager
        let res = store
            .search_similarity(&manager, col, &[1.0, 0.0, 0.0, 0.0], 1, 0.0, None)
            .await
            .unwrap();

        assert!(!res.is_empty(), "La recherche doit remonter le document");
        assert_eq!(res[0].content, "Test d'intégration natif");
    }
}
