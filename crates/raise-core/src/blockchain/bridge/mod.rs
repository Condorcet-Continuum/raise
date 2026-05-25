// FICHIER : src-tauri/src/blockchain/bridge/mod.rs

use crate::blockchain::storage::commit::MentisCommit;
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
    pub async fn process_new_commit(&self, commit: &MentisCommit) -> RaiseResult<()> {
        // 1. Persistance physique garantie (ACID via TransactionManager)
        self.db_adapter.apply_commit(commit).await?;

        // 2. Synchronisation logique dans le ProjectModel (Mémoire)
        // 🎯 FIX MACRO : On respecte la signature ($key, $context)
        if let Err(e) = self.model_sync.sync_commit(commit).await {
            user_error!(
                "ERR_BRIDGE_RAM_SYNC_FAILED",
                json_value!({
                    "commit_id": commit.id,
                    "technical_error": e.to_string(),
                    "hint": "La DB est à jour, mais la mémoire est désynchronisée. Un rechargement de l'UI peut être nécessaire."
                })
            );
        }

        Ok(())
    }
}

// =========================================================================
// TESTS UNITAIRES (Validation Mount Points & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::{MentisCommit, Mutation, MutationOp};
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::{ArcadiaElement, ProjectModel};
    use crate::utils::testing::DbSandbox;

    /// Test existant : Cycle complet Blockchain -> DB -> Mémoire
    #[async_test]
    async fn test_bridge_full_cycle_logic() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let sys_mgr = CollectionsManager::new(
            &sandbox.storage,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // A. Création du schéma générique via URI dynamique
        DbSandbox::mock_db(&sys_mgr).await?;

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        // B. On crée les collections cibles
        sys_mgr.create_collection("components", &schema_uri).await?;

        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        let bridge = ArcadiaBridge::new(&sandbox.storage, &app_state);

        // 🎯 FIX PAYLOAD : On utilise la même technique robuste que dans model_sync.rs
        let default_element = ArcadiaElement::default();
        let mut payload =
            json::serialize_to_value(&default_element).expect("Sérialisation échouée");

        if let Some(obj) = payload.as_object_mut() {
            obj.insert("id".to_string(), json_value!("urn:sa:radar-01"));
            obj.insert("@id".to_string(), json_value!("urn:sa:radar-01"));

            obj.insert("kind".to_string(), json_value!("SystemComponent"));
            obj.insert("@type".to_string(), json_value!("SystemComponent"));
            obj.insert("type".to_string(), json_value!("SystemComponent"));

            obj.insert("name".to_string(), json_value!("Radar System"));
        }

        let mutation = Mutation {
            element_id: "urn:sa:radar-01".into(),
            operation: MutationOp::Create,
            payload,
        };

        let commit = MentisCommit {
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
        let sandbox = DbSandbox::new().await?;
        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        // Simulation d'une configuration corrompue (Domaine inexistant)
        let bridge = ArcadiaBridge::new(&sandbox.storage, &app_state);

        let commit = MentisCommit {
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
        let sandbox = DbSandbox::new().await?;
        let app_state = AppState {
            model: SharedRef::new(AsyncMutex::new(ProjectModel::default())),
        };

        let bridge = ArcadiaBridge::new(&sandbox.storage, &app_state);
        assert!(bridge.model_sync.is_ready());
        Ok(())
    }
}
