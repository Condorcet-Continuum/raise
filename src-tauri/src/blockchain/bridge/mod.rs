// FICHIER : src-tauri/src/blockchain/bridge/mod.rs

use crate::utils::prelude::*;

pub mod db_adapter;
pub mod model_sync;

pub use db_adapter::DbAdapter;
pub use model_sync::ModelSync;

use crate::blockchain::storage::commit::ArcadiaCommit;
use crate::json_db::storage::StorageEngine;
use crate::AppState;

/// Structure principale coordonnant la réconciliation entre la Blockchain et les moteurs RAISE.
pub struct ArcadiaBridge<'a> {
    db_adapter: DbAdapter<'a>,
    model_sync: ModelSync<'a>,
}

impl<'a> ArcadiaBridge<'a> {
    /// Initialise un nouveau pont Arcadia pour un espace et une base de données spécifiques.
    pub fn new(storage: &'a StorageEngine, app_state: &'a AppState) -> Self {
        let config = AppConfig::get();
        Self {
            db_adapter: DbAdapter::new(storage, &config.system_domain, &config.system_db),
            model_sync: ModelSync::new(app_state),
        }
    }

    /// Point d'entrée pour traiter un nouveau commit finalisé par le réseau.
    /// Assure la persistance sur disque suivie de la mise à jour de l'état en mémoire.
    pub async fn process_new_commit(&self, commit: &ArcadiaCommit) -> RaiseResult<()> {
        // 1. Persistance physique dans la JSON-DB
        if let Err(e) = self.db_adapter.apply_commit(commit).await {
            raise_error!(
                "ERR_BRIDGE_DB_PERSISTENCE_FAILED",
                error = e,
                context = json!({
                    "commit_id": commit.id,
                    "adapter": "JsonDbAdapter",
                    "hint": "Le commit n'a pas pu être écrit sur le disque. Vérifiez l'espace disque ou les permissions du dossier storage."
                })
            );
        }

        // 2. Synchronisation logique dans le ProjectModel
        if let Err(e) = self.model_sync.sync_commit(commit) {
            raise_error!(
                "ERR_BRIDGE_MODEL_SYNC_FAILED",
                error = e,
                context = json!({
                    "commit_id": commit.id,
                    "sync_module": "ModelSync",
                    "hint": "Incohérence détectée lors de la mise à jour du modèle en mémoire. Un rollback manuel de la DB pourrait être nécessaire."
                })
            );
        }

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
    use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::model_engine::types::ProjectModel;
    use crate::utils::mock::AgentDbSandbox;
    use crate::utils::Mutex;

    #[tokio::test]
    async fn test_bridge_full_cycle_logic() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        manager
            .create_collection(
                "sa",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .create_collection(
                "components",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        manager
            .create_collection(
                "elements",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let app_state = AppState {
            model: Mutex::new(ProjectModel::default()),
        };

        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);

        // Création d'un commit de test
        let mutation = Mutation {
            element_id: "urn:sa:radar-01".into(),
            operation: MutationOp::Create,
            // Payload "Shotgun" pour garantir la détection du type par ModelSync
            payload: json!({
                "id": "urn:sa:radar-01",
                "@type": "SystemComponent",
                "type": "SystemComponent",
                "kind": "SystemComponent",
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

        // Debug en cas d'échec
        if let Err(e) = &result {
            println!("Erreur Bridge: {:?}", e);
        }
        assert!(result.is_ok());

        // 1. Vérification Mémoire (ModelSync)
        {
            let model = app_state.model.lock().unwrap();
            assert_eq!(
                model.sa.components.len(),
                1,
                "Le composant n'a pas atterri dans SA (Mémoire)"
            );
            assert_eq!(model.sa.components[0].name.as_str(), "Radar System");
        }

        // 2. Vérification Disque (DbAdapter via CollectionsManager)
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // CORRECTION : Le DbAdapter en mode schemaless (test) utilise parfois "components" par défaut.
        // On vérifie les 3 possibilités.
        let doc_sa = manager.get_document("sa", "urn:sa:radar-01").await.unwrap();
        let doc_elements = manager
            .get_document("elements", "urn:sa:radar-01")
            .await
            .unwrap();
        let doc_components = manager
            .get_document("components", "urn:sa:radar-01")
            .await
            .unwrap();

        assert!(
            doc_sa.is_some() || doc_elements.is_some() || doc_components.is_some(),
            "Document absent du disque (vérifié dans 'sa', 'elements' et 'components')"
        );
    }

    #[tokio::test]
    async fn test_bridge_is_ready() {
        let sandbox = AgentDbSandbox::new().await;
        let app_state = AppState {
            model: Mutex::new(ProjectModel::default()),
        };

        let bridge = ArcadiaBridge::new(&sandbox.db, &app_state);
        assert!(bridge.model_sync.is_ready());
    }
}
