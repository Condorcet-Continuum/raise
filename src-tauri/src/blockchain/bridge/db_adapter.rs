// src-tauri/src/blockchain/bridge/db_adapter.rs

use crate::blockchain::storage::commit::{ArcadiaCommit, Mutation, MutationOp};
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::storage::StorageEngine;
use anyhow::{anyhow, Result};
use serde_json::Value;

/// Adaptateur responsable de l'application des commits blockchain dans la JSON-DB.
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
    pub async fn apply_commit(&self, commit: &ArcadiaCommit) -> Result<()> {
        for mutation in &commit.mutations {
            self.apply_mutation(mutation).await?;
        }
        Ok(())
    }

    /// Traduit une mutation individuelle en opération de stockage via le CollectionsManager.
    async fn apply_mutation(&self, mutation: &Mutation) -> Result<()> {
        let collection = self.resolve_collection(&mutation.element_id, &mutation.payload)?;

        match mutation.operation {
            MutationOp::Create | MutationOp::Update => {
                // On prépare les données en injectant l'ID et les métadonnées blockchain
                let mut data = mutation.payload.clone();
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("id".to_string(), Value::String(mutation.element_id.clone()));
                    obj.insert(
                        "_blockchain_sync".to_string(),
                        serde_json::json!({
                            "sync_at": chrono::Utc::now().to_rfc3339(),
                        }),
                    );
                }

                // L'upsert garantit l'idempotence et déclenche la validation JSON-LD/Schéma
                self.manager.upsert_document(&collection, data).await?;
            }
            MutationOp::Delete => {
                self.manager
                    .delete_document(&collection, &mutation.element_id)
                    .await?;
            }
        }
        Ok(())
    }

    /// Détermine la collection cible en fonction de l'URI ou du type de l'élément.
    fn resolve_collection(&self, element_id: &str, payload: &Value) -> Result<String> {
        // 1. Détection par type explicite (@type)
        if let Some(kind) = payload.get("@type").and_then(|v| v.as_str()) {
            return Ok(self.map_type_to_collection(kind));
        }

        // 2. Détection par préfixe d'URN (Fallback)
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

        Err(anyhow!(
            "Impossible de résoudre la collection pour l'ID: {}",
            element_id
        ))
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blockchain::storage::commit::MutationOp;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_db_adapter_upsert_logic() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let adapter = DbAdapter::new(&storage, "test_space", "test_db");

        let mutation = Mutation {
            element_id: "urn:oa:actor-001".to_string(),
            operation: MutationOp::Create,
            payload: json!({
                "@type": "OperationalActor",
                "name": "Pilot"
            }),
        };

        // Application de la mutation
        let res = adapter.apply_mutation(&mutation).await;
        assert!(res.is_ok());

        // Vérification via le manager (persistance confirmée)
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        let doc = manager
            .get_document("actors", "urn:oa:actor-001")
            .await
            .unwrap();

        assert!(doc.is_some());
        assert_eq!(doc.unwrap()["name"], "Pilot");
    }
}
