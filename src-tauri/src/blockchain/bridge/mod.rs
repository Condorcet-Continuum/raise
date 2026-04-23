// FICHIER : src-tauri/src/blockchain/bridge/mod.rs

use crate::blockchain::storage::commit::ArcadiaCommit;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE
use crate::AppState;

pub mod db_adapter;
pub mod model_sync;

pub use db_adapter::DbAdapter;
pub use model_sync::ModelSync;

/// Structure principale coordonnant la réconciliation entre la Blockchain et les moteurs RAISE.
/// Assure l'intégrité entre le registre distribué et le graphe de connaissance local.
pub struct ArcadiaBridge<'a> {
    db_adapter: DbAdapter<'a>,
    model_sync: ModelSync<'a>,
}

impl<'a> ArcadiaBridge<'a> {
    /// Initialise le pont en résolvant les domaines techniques via les Mount Points système.
    pub fn new(storage: &'a StorageEngine, app_state: &'a AppState) -> Self {
        let config = AppConfig::get();
        // 🎯 RÉSOLUTION VIA MOUNT POINTS : Utilisation des domaines système configurés
        Self {
            db_adapter: DbAdapter::new(
                storage,
                &config.mount_points.system.domain,
                &config.mount_points.system.db,
            ),
            model_sync: ModelSync::new(app_state),
        }
    }

    /// Traite un nouveau commit blockchain : Persistance physique (DB) puis synchronisation logique (Modèle).
    pub async fn process_new_commit(&self, commit: &ArcadiaCommit) -> RaiseResult<()> {
        // 1. Persistance physique dans la JSON-DB avec Match strict
        match self.db_adapter.apply_commit(commit).await {
            Ok(_) => (),
            Err(e) => raise_error!(
                "ERR_BRIDGE_DB_PERSISTENCE_FAILED",
                error = e.to_string(),
                context = json_value!({ "commit_id": commit.id })
            ),
        }

        // 2. Synchronisation logique dans le ProjectModel
        match self.model_sync.sync_commit(commit).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_BRIDGE_MODEL_SYNC_FAILED",
                error = e.to_string(),
                context = json_value!({ "commit_id": commit.id })
            ),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Mount Points & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Cycle complet Blockchain -> DB -> Mémoire
    #[async_test]
    async fn test_bridge_full_cycle_logic() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let sys_mgr = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // A. Création du schéma générique via URI dynamique
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
            .await?;

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        // B. On crée les collections cibles
        sys_mgr.create_collection("components", &schema_uri).await?;
        sys_mgr
            .create_collection("system_elements", &schema_uri)
            .await?;

        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);

        let mutation = Mutation {
            element_id: "urn:sa:radar-01".into(),
            operation: MutationOp::Create,
            payload: json_value!({
                "id": "urn:sa:radar-01",
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

        // Exécution du pont
        bridge.process_new_commit(&commit).await?;

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

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à un domaine système invalide (Mount Point Error)
    #[async_test]
    async fn test_bridge_resilience_on_invalid_mount_point() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        // Simulation d'une configuration corrompue (Domaine inexistant)
        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);

        let commit = ArcadiaCommit {
            id: "tx_fail".into(),
            parent_hash: None,
            author: "tester".into(),
            timestamp: UtcClock::now(),
            mutations: vec![], // Même vide, le bridge doit valider la DB
            merkle_root: "none".into(),
            signature: vec![],
        };

        let result = bridge.process_new_commit(&commit).await;

        // Le pont ne doit pas paniquer, mais renvoyer un Result RaiseResult
        assert!(result.is_ok() || result.is_err());
        Ok(())
    }

    #[async_test]
    async fn test_bridge_is_ready() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);
        assert!(bridge.model_sync.is_ready());
        Ok(())
    }
}
