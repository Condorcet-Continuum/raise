// FICHIER : src-tauri/src/ai/memory/mod.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*; // 🎯 Façade Unique

pub mod native_store;

#[derive(Debug, Clone, Serializable, Deserializable)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub metadata: JsonValue,
    pub vectors: Option<Vec<f32>>,
}

#[async_interface]
pub trait VectorStore: Send + Sync {
    /// Initialise une collection vectorielle en s'assurant de la présence du schéma technique.
    async fn init_collection(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        vector_size: u64,
    ) -> RaiseResult<()>;

    /// Ajoute des documents de manière synchronisée entre le moteur tensoriel et JSON-DB.
    async fn add_documents(
        &self,
        manager: &CollectionsManager<'_>,
        collection_name: &str,
        records: Vec<MemoryRecord>,
    ) -> RaiseResult<()>;

    /// Recherche par similarité cosinus avec filtrage hybride (Vecteurs + Métadonnées DB).
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

// =========================================================================
// TESTS D'INTÉGRATION (Rigueur Façade, Mount Points & Résilience)
// =========================================================================
#[cfg(test)]
mod integration_tests {
    use super::{MemoryRecord, VectorStore};
    use crate::ai::memory::native_store::NativeLocalStore;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::prelude::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    /// Test existant : Cycle de vie complet via Sandbox
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_native_lifecycle() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let device = ComputeHardware::Cpu;
        let store_dir = sandbox.domain_root.join("vector_store");
        let store = NativeLocalStore::new(&store_dir, &device);

        let col = "integ_test_collection";

        // Résolution dynamique du schéma via les points de montage système
        let schema_uri = format!(
            "db://{}/{}/schemas/v2/agents/memory/vector_store_record.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        manager.create_collection(col, &schema_uri).await?;

        // Initialisation du store vectoriel
        store.init_collection(&manager, col, 4).await?;

        let rec = MemoryRecord {
            id: UniqueId::new_v4().to_string(),
            content: "Test d'intégration natif".into(),
            metadata: json_value!({"env": "test"}),
            vectors: Some(vec![1.0, 0.0, 0.0, 0.0]),
        };

        // Persistance et indexation
        store
            .add_documents(&manager, col, vec![rec.clone()])
            .await?;

        // Recherche sémantique
        let res = store
            .search_similarity(&manager, col, &[1.0, 0.0, 0.0, 0.0], 1, 0.0, None)
            .await?;

        assert!(!res.is_empty(), "La recherche doit remonter le document");
        assert_eq!(res[0].content, "Test d'intégration natif");

        Ok(())
    }

    ///  Résilience face à un domaine inexistant (Mount Point Error)
    /// 🎯 NOUVEAU TEST : Résilience face à un domaine inexistant (Mount Point Error)
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_memory_resilience_invalid_domain() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;

        // On crée un manager pointant vers un domaine non initialisé
        let manager = CollectionsManager::new(&sandbox.db, "ghost_domain", "ghost_db");
        let store = NativeLocalStore::new(&sandbox.domain_root.join("fail"), &ComputeHardware::Cpu);

        // 1. L'initialisation est "Lazy/Résiliente" et ne crashera pas
        let _ = store.init_collection(&manager, "any", 384).await;

        // 2. Par contre, l'écriture (upsert) DOIT être strictement interceptée !
        let rec = MemoryRecord {
            id: "ghost_1".into(),
            content: "Donnée fantôme".into(),
            metadata: json_value!({}),
            vectors: Some(vec![0.0; 384]),
        };

        // L'interaction avec JSON-DB sera rejetée avec une erreur structurée
        let result = store.add_documents(&manager, "any", vec![rec]).await;

        // 🎯 FIX : Utilisation du standard de test de l'application (e.to_string())
        assert!(
        result.is_err(),
        "Le moteur de mémoire aurait dû lever une erreur pour domaine invalide lors de l'insertion"
    );

        if let Err(e) = result {
            assert!(
                e.to_string().contains("ERR_DB"),
                "L'erreur remontée n'est pas de type DB : {}",
                e
            );
        }

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Étanchéité des collections (Multi-tenant)
    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_memory_collection_isolation() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let store = NativeLocalStore::new(&sandbox.domain_root, &ComputeHardware::Cpu);

        // Init deux collections distinctes
        store.init_collection(&manager, "col_a", 2).await?;
        store.init_collection(&manager, "col_b", 2).await?;

        let rec_a = MemoryRecord {
            id: "A".into(),
            content: "Data A".into(),
            metadata: json_value!({}),
            vectors: Some(vec![1.0, 0.0]),
        };
        let rec_b = MemoryRecord {
            id: "B".into(),
            content: "Data B".into(),
            metadata: json_value!({}),
            vectors: Some(vec![1.0, 0.0]),
        };

        store.add_documents(&manager, "col_a", vec![rec_a]).await?;
        store.add_documents(&manager, "col_b", vec![rec_b]).await?;

        // La recherche dans A ne doit JAMAIS remonter B
        let res = store
            .search_similarity(&manager, "col_a", &[1.0, 0.0], 10, 0.5, None)
            .await?;
        assert!(
            res.iter().all(|r| r.id == "A"),
            "Fuite de données entre collections détectée !"
        );

        Ok(())
    }
}
