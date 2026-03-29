// FICHIER : src-tauri/src/blockchain/bridge/mod.rs

use crate::blockchain::storage::commit::ArcadiaCommit;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*;
use crate::AppState;

pub mod db_adapter;
pub mod model_sync;

pub use db_adapter::DbAdapter;
pub use model_sync::ModelSync;

/// Structure principale coordonnant la réconciliation entre la Blockchain et les moteurs RAISE.
pub struct ArcadiaBridge<'a> {
    db_adapter: DbAdapter<'a>,
    model_sync: ModelSync<'a>,
}

impl<'a> ArcadiaBridge<'a> {
    pub fn new(storage: &'a StorageEngine, app_state: &'a AppState) -> Self {
        let config = AppConfig::get();
        Self {
            db_adapter: DbAdapter::new(storage, &config.system_domain, &config.system_db),
            model_sync: ModelSync::new(app_state),
        }
    }

    pub async fn process_new_commit(&self, commit: &ArcadiaCommit) -> RaiseResult<()> {
        // 1. Persistance physique dans la JSON-DB
        if let Err(e) = self.db_adapter.apply_commit(commit).await {
            raise_error!(
                "ERR_BRIDGE_DB_PERSISTENCE_FAILED",
                error = e,
                context = json_value!({ "commit_id": commit.id })
            );
        }

        // 2. Synchronisation logique dans le ProjectModel
        match self.model_sync.sync_commit(commit).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_BRIDGE_MODEL_SYNC_FAILED",
                error = e,
                context = json_value!({ "commit_id": commit.id })
            ),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_bridge_full_cycle_logic() {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let sys_mgr =
            CollectionsManager::new(&sandbox.db, &config.system_domain, &config.system_db);

        // A. Création du schéma générique
        let _ = sys_mgr.create_collection("schemas", "").await;
        sys_mgr
            .insert_raw(
                "schemas",
                &json_value!({
                    "_id": "v1/db/generic.schema.json",
                    "type": "jsonschema",
                    "content": {}
                }),
            )
            .await
            .unwrap();

        let schema_uri = "db://_system/_system/schemas/v1/db/generic.schema.json";

        // B. On crée les collections que le DbAdapter va cibler
        sys_mgr
            .create_collection("components", schema_uri)
            .await
            .unwrap();
        sys_mgr
            .create_collection("system_elements", schema_uri)
            .await
            .unwrap();

        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);

        let mutation = Mutation {
            element_id: "urn:sa:radar-01".into(),
            operation: MutationOp::Create,
            payload: json_value!({
                "id": "urn:sa:radar-01",  // 🎯 FIX FINAL : "id" au lieu de "_id" pour satisfaire serde
                "@type": "SystemComponent",
                "type": "SystemComponent",
                "name": "Radar System"
            }),
        };

        let commit = ArcadiaCommit {
            id: "tx_123".into(),
            parent_hash: None,
            author: "dev".into(),
            timestamp: UtcClock::now(),
            mutations: vec![mutation],
            merkle_root: "root".into(),
            signature: vec![],
        };

        let result = bridge.process_new_commit(&commit).await;
        assert!(result.is_ok(), "Le pont a échoué : {:?}", result.err());

        // Vérification Pure Graph (Mémoire)
        {
            let model = app_state.model.lock().await;
            let sa_components = model.get_collection("sa", "components");
            assert_eq!(
                sa_components.len(),
                1,
                "Le composant doit être synchronisé en mémoire"
            );
            assert_eq!(sa_components[0].name.as_str(), "Radar System");
        }
    }

    #[async_test]
    async fn test_bridge_is_ready() {
        let sandbox = AgentDbSandbox::new().await;
        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);
        assert!(bridge.model_sync.is_ready());
    }
}
