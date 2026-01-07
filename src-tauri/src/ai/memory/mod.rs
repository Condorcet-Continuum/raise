pub mod leann_store;
pub mod qdrant_store;
pub mod tests;
use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Représente une "pensée" ou un document stocké en mémoire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryRecord {
    pub id: String,
    pub content: String,
    pub metadata: serde_json::Value,
    /// Le vecteur est Optionnel car on ne le récupère pas forcément à la lecture
    pub vectors: Option<Vec<f32>>,
}

/// Interface générique pour le stockage vectoriel (Pattern Strategy)
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Prépare la collection (équivalent d'une table SQL)
    async fn init_collection(&self, collection_name: &str, vector_size: u64) -> Result<()>;

    /// Ajoute ou met à jour des souvenirs
    async fn add_documents(&self, collection_name: &str, records: Vec<MemoryRecord>) -> Result<()>;

    /// Recherche sémantique
    async fn search_similarity(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
        score_threshold: f32,
    ) -> Result<Vec<MemoryRecord>>;
}
