use crate::ai::memory::{qdrant_store::QdrantMemory, MemoryRecord, VectorStore};
use crate::ai::nlp::embeddings::EmbeddingEngine;
use anyhow::{Context, Result};
use serde_json::json;
use uuid::Uuid;

/// Le Retrouveur Sémantique (RAG)
/// Cherche dans la base de connaissance vectorielle (Documentation, Specs...)
pub struct RagRetriever {
    memory: QdrantMemory,
    embedder: EmbeddingEngine,
    collection_name: String,
}

impl RagRetriever {
    /// Initialise la connexion à Qdrant et charge le modèle d'embedding
    pub async fn new(qdrant_url: &str) -> Result<Self> {
        let memory = QdrantMemory::new(qdrant_url)
            .context("Échec connexion Qdrant (Docker est-il lancé ?)")?;

        // Initialisation du moteur NLP (FastEmbed ou Candle selon config)
        let embedder = EmbeddingEngine::new().context("Échec init Embedder")?;

        let collection_name = "raise_knowledge_base".to_string();

        // On s'assure que la collection existe (taille 384 = BGE-Small standard)
        memory.init_collection(&collection_name, 384).await?;

        Ok(Self {
            memory,
            embedder,
            collection_name,
        })
    }

    /// Indexe un texte brut dans la mémoire (Documentation, Note, etc.)
    pub async fn index_document(&mut self, content: &str, source: &str) -> Result<()> {
        // 1. Vectorisation
        let vector = self.embedder.embed_query(content)?;

        // 2. Création de l'enregistrement
        let record = MemoryRecord {
            id: Uuid::new_v4().to_string(),
            content: content.to_string(),
            metadata: json!({
                "source": source,
                "ingested_at": chrono::Utc::now().to_rfc3339()
            }),
            vectors: Some(vector),
        };

        // 3. Envoi à Qdrant
        self.memory
            .add_documents(&self.collection_name, vec![record])
            .await?;

        Ok(())
    }

    /// Recherche les documents les plus pertinents sémantiquement
    pub async fn retrieve(&mut self, query: &str, limit: u64) -> Result<String> {
        // 1. Vectorisation de la question
        let query_vector = self.embedder.embed_query(query)?;

        // 2. Recherche vectorielle
        let results = self
            .memory
            .search_similarity(&self.collection_name, &query_vector, limit, 0.4)
            .await?;

        if results.is_empty() {
            return Ok(String::new());
        }

        // 3. Formatage pour le contexte LLM
        let mut context_str = String::from("### DOCUMENTATION PERTINENTE (RAG) ###\n");
        for (i, doc) in results.iter().enumerate() {
            let source = doc
                .metadata
                .get("source")
                .and_then(|v| v.as_str())
                .unwrap_or("Inconnu");
            context_str.push_str(&format!("Source [{}]: {}\n", source, doc.content));
            if i < results.len() - 1 {
                context_str.push_str("---\n");
            }
        }

        Ok(context_str)
    }
}
