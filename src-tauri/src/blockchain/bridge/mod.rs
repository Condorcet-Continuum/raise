// src-tauri/src/blockchain/bridge/mod.rs

pub mod db_adapter;
pub mod model_sync;

pub use db_adapter::DbAdapter;
pub use model_sync::ModelSync;

use crate::blockchain::storage::commit::ArcadiaCommit;
use crate::json_db::storage::StorageEngine;
use crate::AppState;
use anyhow::{Context, Result};

/// Structure principale coordonnant la réconciliation entre la Blockchain et les moteurs RAISE.
pub struct ArcadiaBridge<'a> {
    db_adapter: DbAdapter<'a>,
    model_sync: ModelSync<'a>,
}

impl<'a> ArcadiaBridge<'a> {
    /// Initialise un nouveau pont Arcadia pour un espace et une base de données spécifiques.
    pub fn new(storage: &'a StorageEngine, app_state: &'a AppState) -> Self {
        Self {
            db_adapter: DbAdapter::new(storage, "un2", "_system"),
            model_sync: ModelSync::new(app_state),
        }
    }

    /// Point d'entrée pour traiter un nouveau commit finalisé par le réseau.
    /// Assure la persistance sur disque suivie de la mise à jour de l'état en mémoire.
    pub async fn process_new_commit(&self, commit: &ArcadiaCommit) -> Result<()> {
        // 1. Persistance physique dans la JSON-DB
        self.db_adapter
            .apply_commit(commit)
            .await
            .context("Échec de l'application du commit dans la JSON-DB via le DbAdapter")?;

        // 2. Synchronisation logique dans le ProjectModel
        self.model_sync
            .sync_commit(commit)
            .context("Échec de la synchronisation du ProjectModel en mémoire via le ModelSync")?;

        #[cfg(debug_assertions)]
        println!(
            "🚀 [ArcadiaBridge] Commit {} traité avec succès.",
            commit.id
        );

        Ok(())
    }
}

// --- TESTS UNITAIRES ---

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::{Mutation, MutationOp};
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::model_engine::types::ProjectModel;
    use serde_json::json;
    use std::sync::Mutex;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_bridge_full_cycle_logic() {
        // Setup Environnement
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let app_state = AppState {
            model: Mutex::new(ProjectModel::default()),
        };

        let bridge = ArcadiaBridge::new(&storage, &app_state);

        // Création d'un commit de test
        let mutation = Mutation {
            element_id: "urn:sa:radar-01".into(),
            operation: MutationOp::Create,
            payload: json!({
                "id": "urn:sa:radar-01",
                "@type": "SystemComponent",
                "name": "Radar System"
            }),
        };

        let commit = ArcadiaCommit {
            id: "tx_123".into(),
            parent_hash: None,
            author: "dev_key".into(),
            timestamp: chrono::Utc::now(),
            mutations: vec![mutation],
            merkle_root: "root".into(),
            signature: vec![],
        };

        // Exécution du pont
        let result = bridge.process_new_commit(&commit).await;
        assert!(result.is_ok());

        // 1. Vérification Mémoire (ModelSync)
        {
            let model = app_state.model.lock().unwrap();
            assert_eq!(model.sa.components.len(), 1);
            assert_eq!(model.sa.components[0].name.as_str(), "Radar System");
        }

        // 2. Vérification Disque (DbAdapter via CollectionsManager)
        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage, "un2", "_system",
        );
        let doc = manager
            .get_document("components", "urn:sa:radar-01")
            .await
            .unwrap();
        assert!(doc.is_some());
    }

    #[test]
    fn test_bridge_is_ready() {
        let dir = tempdir().unwrap();
        let storage = StorageEngine::new(JsonDbConfig::new(dir.path().to_path_buf()));
        let app_state = AppState {
            model: Mutex::new(ProjectModel::default()),
        };

        let bridge = ArcadiaBridge::new(&storage, &app_state);
        assert!(bridge.model_sync.is_ready());
    }
}
