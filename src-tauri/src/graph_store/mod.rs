pub mod surreal_impl;

use self::surreal_impl::SurrealClient;
use anyhow::Result;
use std::path::PathBuf;

#[derive(Clone)]
pub struct GraphStore {
    backend: SurrealClient,
}

impl GraphStore {
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        let backend = SurrealClient::init(storage_path).await?;
        Ok(Self { backend })
    }

    /// Indexe une entité. On ignore la valeur de retour (l'ancien document) avec `let _`.
    pub async fn index_entity(
        &self,
        collection: &str,
        id: &str,
        data: serde_json::Value,
    ) -> Result<()> {
        let _ = self.backend.upsert_node(collection, id, data).await?;
        Ok(())
    }

    pub async fn remove_entity(&self, collection: &str, id: &str) -> Result<()> {
        self.backend.delete_node(collection, id).await
    }

    pub async fn link_entities(
        &self,
        from: (&str, &str),
        relation: &str,
        to: (&str, &str),
    ) -> Result<()> {
        self.backend.create_edge(from, relation, to).await
    }

    pub fn backend(&self) -> &SurrealClient {
        &self.backend
    }
}
