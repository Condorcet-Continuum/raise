// src-tauri/src/blockchain/bridge/db_adapter.rs

use crate::blockchain::storage::commit::{MentisCommit, MutationOp};
use crate::json_db::storage::StorageEngine;
use crate::json_db::transactions::manager::TransactionManager;
use crate::json_db::transactions::TransactionRequest;
use crate::utils::prelude::*;

/// Adaptateur responsable de l'application des commits blockchain dans la JSON-DB.
/// Assure la synchronisation ACID entre le registre distribué et le stockage local.
pub struct DbAdapter<'a> {
    storage: &'a StorageEngine,
    space: String,
    db: String,
}

impl<'a> DbAdapter<'a> {
    /// Crée un nouvel adaptateur pour un espace et une base de données spécifiques.
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            storage,
            space: space.to_string(),
            db: db.to_string(),
        }
    }

    /// Applique l'intégralité d'un commit Mentis de manière ATOMIQUE (ACID).
    pub async fn apply_commit(&self, commit: &MentisCommit) -> RaiseResult<()> {
        let tm = TransactionManager::new(self.storage, &self.space, &self.db);
        let mut requests = Vec::new();

        for mutation in &commit.mutations {
            let collection = self.resolve_collection(&mutation.element_id, &mutation.payload)?;

            match mutation.operation {
                MutationOp::Create | MutationOp::Update => {
                    let mut data = mutation.payload.clone();

                    match data.as_object_mut() {
                        Some(obj) => {
                            obj.insert(
                                "_blockchain_sync".to_string(),
                                json_value!({
                                    "sync_at": UtcClock::now().to_rfc3339(),
                                    "commit_id": commit.id,
                                    "op": format!("{:?}", mutation.operation)
                                }),
                            );
                        }
                        None => raise_error!(
                            "ERR_BLOCKCHAIN_PAYLOAD_INVALID",
                            error = "Le payload de la mutation doit être un objet JSON valide.",
                            context = json_value!({ "element_id": mutation.element_id })
                        ),
                    }

                    requests.push(TransactionRequest::Upsert {
                        collection,
                        id: Some(mutation.element_id.clone()),
                        handle: None,
                        document: data,
                    });
                }
                MutationOp::Delete => {
                    requests.push(TransactionRequest::Delete {
                        collection,
                        id: mutation.element_id.clone(),
                    });
                }
            }
        }

        // Exécution atomique via le moteur JSON_DB
        match tm.execute_smart(requests).await {
            Ok(_) => Ok(()),
            Err(e) => raise_error!(
                "ERR_BLOCKCHAIN_COMMIT_APPLY_FAILED",
                error = format!("Échec transactionnel du commit : {}", e),
                context = json_value!({ "commit_id": commit.id })
            ),
        }
    }

    /// Détermine la collection cible en fonction de l'URI ou du type de l'élément.
    fn resolve_collection(&self, element_id: &str, payload: &JsonValue) -> RaiseResult<String> {
        // 1. Détection par type explicite (@type)
        if let Some(kind) = payload.get("@type").and_then(|v| v.as_str()) {
            return Ok(self.map_type_to_collection(kind));
        }

        // 2. Détection par préfixe d'URN
        if element_id.starts_with("urn:oa:") {
            return Ok("actors".to_string());
        }
        if element_id.starts_with("urn:sa:") {
            return Ok("components".to_string());
        }
        if element_id.starts_with("urn:la:") {
            return Ok("components".to_string());
        }
        if element_id.starts_with("urn:pa:") {
            return Ok("components".to_string());
        }

        // 🎯 FIX ANTI-FORK : Si on ne reconnait pas l'URN, on ne crashe pas. On isole.
        user_warn!(
            "⚠️ [Bridge] Type inconnu pour {}, assignation à 'elements_orphans'",
            element_id
        );
        Ok("elements_orphans".to_string())
    }

    /// Mappe les types Mentis sémantiques vers les noms de collections physiques.
    fn map_type_to_collection(&self, kind: &str) -> String {
        match kind {
            "OperationalActor" | "OperationalEntity" => "actors".to_string(),
            "SystemComponent" | "LogicalComponent" | "PhysicalComponent" => {
                "components".to_string()
            }
            "SystemFunction" | "LogicalFunction" | "PhysicalFunction" => "functions".to_string(),
            _ => "elements".to_string(),
        }
    }
}

// =========================================================================
// TESTS UNITAIRES
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::Mutation;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::utils::testing::DbSandbox;

    #[async_test]
    async fn test_db_adapter_apply_commit_success() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let storage = &sandbox.storage;

        // Préparation de la collection via le manager
        let col_mgr = CollectionsManager::new(storage, space, db);
        DbSandbox::mock_db(&col_mgr).await?;
        col_mgr
            .create_collection(
                "actors",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        let adapter = DbAdapter::new(storage, space, db);

        let commit = MentisCommit {
            id: "tx_123".to_string(),
            parent_hash: None,
            author: "author_01".to_string(),
            timestamp: UtcClock::now(),
            mutations: vec![Mutation {
                element_id: "urn:oa:actor-001".to_string(),
                operation: MutationOp::Create,
                payload: json_value!({
                    "@type": "OperationalActor",
                    "name": "Pilot"
                }),
            }],
            merkle_root: "root".to_string(),
            signature: vec![],
        };

        match adapter.apply_commit(&commit).await {
            Ok(_) => {}
            Err(e) => panic!("L'application du commit a échoué : {:?}", e),
        }

        // Vérification de la persistance
        let doc = col_mgr
            .get("actors", "urn:oa:actor-001")
            .await?
            .expect("Le document devrait exister");
        assert_eq!(doc["name"], "Pilot");
        assert!(doc.get("_blockchain_sync").is_some());

        Ok(())
    }

    #[async_test]
    async fn test_db_adapter_invalid_payload_rejection() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let adapter = DbAdapter::new(&sandbox.storage, space, db);

        let commit = MentisCommit {
            id: "tx_fail".to_string(),
            parent_hash: None,
            author: "author_01".to_string(),
            timestamp: UtcClock::now(),
            mutations: vec![Mutation {
                element_id: "urn:test:01".to_string(),
                operation: MutationOp::Create,
                payload: json_value!(["not", "an", "object"]), // Array au lieu de Object
            }],
            merkle_root: "".to_string(),
            signature: vec![],
        };

        match adapter.apply_commit(&commit).await {
            Ok(_) => panic!("L'adaptateur aurait dû rejeter un payload non-objet"),
            Err(e) => {
                let err_str = e.to_string();
                assert!(err_str.contains("ERR_BLOCKCHAIN_PAYLOAD_INVALID"));
            }
        }
        Ok(())
    }

    #[async_test]
    async fn test_db_adapter_fallback_orphan_urn() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await?;
        let space = &sandbox.config.mount_points.system.domain;
        let db = &sandbox.config.mount_points.system.db;
        let adapter = DbAdapter::new(&sandbox.storage, space, db);

        // Une URN qui ne matche aucun préfixe connu et sans @type
        let payload = json_value!({ "data": "unknown" });

        match adapter.resolve_collection("urn:unknown:999", &payload) {
            Ok(col) => assert_eq!(col, "elements_orphans", "Doit fallback sur les orphelins"),
            Err(_) => panic!("Ne doit plus lever d'erreur sur un type inconnu"),
        }
        Ok(())
    }
}
