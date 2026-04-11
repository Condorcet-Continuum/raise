// src-tauri/src/blockchain/bridge/db_adapter.rs

use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

/// Adaptateur responsable de l'application des commits blockchain dans la JSON-DB.
/// Assure la synchronisation entre le registre distribué et le graphe de connaissance local.
pub struct DbAdapter<'a> {
    manager: CollectionsManager<'a>,
}

impl<'a> DbAdapter<'a> {
    /// Crée un nouvel adaptateur pour un espace et une base de données spécifiques.
    pub fn new(storage: &'a StorageEngine, space: &str, db: &str) -> Self {
        Self {
            manager: CollectionsManager::new(storage, space, db),
        }
    }

    /// Applique l'intégralité d'un commit Arcadia dans la base de données locale.
    pub async fn apply_commit(&self, commit: &ArcadiaCommit) -> RaiseResult<()> {
        for mutation in &commit.mutations {
            match self.apply_mutation(mutation).await {
                Ok(_) => (),
                Err(e) => raise_error!(
                    "ERR_BLOCKCHAIN_COMMIT_APPLY_FAILED",
                    error = e.to_string(),
                    context = json_value!({ "element_id": mutation.element_id })
                ),
            }
        }
        Ok(())
    }

    /// Traduit une mutation individuelle en opération de stockage via le CollectionsManager.
    async fn apply_mutation(&self, mutation: &Mutation) -> RaiseResult<()> {
        let collection = self.resolve_collection(&mutation.element_id, &mutation.payload)?;

        match mutation.operation {
            MutationOp::Create | MutationOp::Update => {
                // On prépare les données en injectant l'ID et les métadonnées blockchain
                let mut data = mutation.payload.clone();

                match data.as_object_mut() {
                    Some(obj) => {
                        obj.insert(
                            "_id".to_string(),
                            JsonValue::String(mutation.element_id.clone()),
                        );
                        obj.insert(
                            "_blockchain_sync".to_string(),
                            json_value!({
                                "sync_at": UtcClock::now().to_rfc3339(),
                                "op": format!("{:?}", mutation.operation)
                            }),
                        );
                    }
                    None => raise_error!(
                        "ERR_BLOCKCHAIN_PAYLOAD_INVALID",
                        error = "Le payload de la mutation doit être un objet JSON valide."
                    ),
                }

                // L'upsert garantit l'idempotence et déclenche la validation de schéma
                match self.manager.upsert_document(&collection, data).await {
                    Ok(_) => Ok(()),
                    Err(e) => raise_error!("ERR_DB_UPSERT_FAILED", error = e.to_string()),
                }
            }
            MutationOp::Delete => {
                match self
                    .manager
                    .delete_document(&collection, &mutation.element_id)
                    .await
                {
                    Ok(_) => Ok(()),
                    Err(e) => raise_error!("ERR_DB_DELETE_FAILED", error = e.to_string()),
                }
            }
        }
    }

    /// Détermine la collection cible en fonction de l'URI ou du type de l'élément.
    fn resolve_collection(&self, element_id: &str, payload: &JsonValue) -> RaiseResult<String> {
        // 1. Détection par type explicite (@type)
        if let Some(kind) = payload.get("@type").and_then(|v| v.as_str()) {
            return Ok(self.map_type_to_collection(kind));
        }

        // 2. Détection par préfixe d'URN (Fallback déterministe)
        if element_id.starts_with("urn:oa:") {
            return Ok("operational_elements".to_string());
        }
        if element_id.starts_with("urn:sa:") {
            return Ok("system_elements".to_string());
        }
        if element_id.starts_with("urn:la:") {
            return Ok("logical_elements".to_string());
        }
        if element_id.starts_with("urn:pa:") {
            return Ok("physical_elements".to_string());
        }

        raise_error!(
            "ERR_DB_COLLECTION_RESOLUTION_FAIL",
            error = format!(
                "Impossible de mapper l'ID '{}' vers une collection physique.",
                element_id
            ),
            context = json_value!({ "element_id": element_id })
        )
    }

    /// Mappe les types Arcadia sémantiques vers les noms de collections physiques.
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
// TESTS UNITAIRES (Validation Mount Points & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::MutationOp;
    use crate::utils::testing::AgentDbSandbox;

    /// Test existant : Validation de la logique Upsert
    #[async_test]
    async fn test_db_adapter_upsert_logic() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        let adapter = DbAdapter::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mutation = Mutation {
            element_id: "urn:oa:actor-001".to_string(),
            operation: MutationOp::Create,
            payload: json_value!({
                "@type": "OperationalActor",
                "name": "Pilot"
            }),
        };

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        manager.create_collection("actors", &schema_uri).await?;

        // Application de la mutation
        adapter.apply_mutation(&mutation).await?;

        // Vérification via le manager (persistance confirmée)
        let doc = match manager.get_document("actors", "urn:oa:actor-001").await? {
            Some(d) => d,
            None => panic!("Le document aurait dû être persisté par l'adapter"),
        };

        assert_eq!(doc["name"], "Pilot");
        assert!(doc.get("_blockchain_sync").is_some());
        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à un payload invalide (non-objet)
    #[async_test]
    async fn test_db_adapter_resilience_invalid_payload() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let adapter = DbAdapter::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mutation = Mutation {
            element_id: "urn:oa:fail".to_string(),
            operation: MutationOp::Create,
            payload: json_value!(["not", "an", "object"]),
        };

        let result = adapter.apply_mutation(&mutation).await;
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_BLOCKCHAIN_PAYLOAD_INVALID");
                Ok(())
            }
            _ => panic!("L'adapter aurait dû lever ERR_BLOCKCHAIN_PAYLOAD_INVALID"),
        }
    }

    /// 🎯 NOUVEAU TEST : Inférence Mount Point (System Domain)
    #[async_test]
    async fn test_db_adapter_mount_point_resolution() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        let adapter = DbAdapter::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Vérifie que l'adapter pointe bien vers la partition système configurée
        assert_eq!(adapter.manager.space, config.mount_points.system.domain);
        assert_eq!(adapter.manager.db, config.mount_points.system.db);
        Ok(())
    }
}
