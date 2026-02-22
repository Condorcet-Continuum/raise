use crate::ai::memory::{candle_store::CandleLocalStore, MemoryRecord, VectorStore};
use crate::ai::nlp::{embeddings::EmbeddingEngine, splitting};
use crate::utils::{io::PathBuf, prelude::*, Uuid};
use candle_core::Device;

pub struct RagRetriever {
    backend: CandleLocalStore, // 🎯 Connexion directe et exclusive au moteur natif
    embedder: EmbeddingEngine,
    collection_name: String,
}

impl RagRetriever {
    /// Initialise le RAG en se basant EXCLUSIVEMENT sur la configuration globale
    pub async fn new() -> Result<Self> {
        let config = AppConfig::get();
        let storage_path = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_default_domain"));

        Self::new_internal(storage_path).await
    }

    /// Constructeur interne pour injecter le path (Idéal pour les tests)
    pub async fn new_internal(storage_path: PathBuf) -> Result<Self> {
        let embedder = EmbeddingEngine::new()
            .map_err(|e| AppError::Ai(format!("Échec init Embedder: {}", e)))?;
        let collection_name = "raise_knowledge_base".to_string();

        println!("📚 [RAG] Initialisation du backend : CANDLE (100% Natif)");

        let device = Device::Cpu;
        let store_dir = storage_path.join("vector_store");
        let memory = CandleLocalStore::new(&store_dir, &device);
        memory.init_collection(&collection_name, 384).await?;
        memory.load().await?; // Charge l'historique s'il existe

        Ok(Self {
            backend: memory,
            embedder,
            collection_name,
        })
    }

    pub async fn index_document(&mut self, content: &str, source: &str) -> Result<usize> {
        let chunks = splitting::split_text_into_chunks(content, 512);
        if chunks.is_empty() {
            return Ok(0);
        }

        let vectors = self.embedder.embed_batch(chunks.clone())?;
        let ingest_time = Utc::now().to_rfc3339();

        let mut records = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            records.push(MemoryRecord {
                id: Uuid::new_v4().to_string(),
                content: chunk.clone(),
                metadata: json!({
                    "source": source,
                    "chunk_index": i,
                    "total_chunks": chunks.len(),
                    "ingested_at": ingest_time
                }),
                vectors: Some(vectors[i].clone()),
            });
        }

        // 🎯 L'ajout et la sauvegarde se font directement sur le backend
        self.backend
            .add_documents(&self.collection_name, records)
            .await?;
        self.backend.save().await?; // Persistance immédiate

        Ok(chunks.len())
    }

    pub async fn retrieve(&mut self, query: &str, limit: u64) -> Result<String> {
        let query_vector = self.embedder.embed_query(query)?;

        // Seuil ajusté pour le modèle multilingue
        let min_similarity = 0.65;

        let docs = self
            .backend
            .search_similarity(
                &self.collection_name,
                &query_vector,
                limit,
                min_similarity,
                None,
            )
            .await?;

        let raw_results: Vec<(String, String)> = docs
            .into_iter()
            .map(|d| {
                let src = d
                    .metadata
                    .get("source")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                (src, d.content)
            })
            .collect();

        if raw_results.is_empty() {
            return Ok(String::new());
        }

        let mut context_str = String::from("### DOCUMENTATION PERTINENTE (RAG) ###\n");
        for (i, (source, content)) in raw_results.iter().enumerate() {
            context_str.push_str(&format!("Source [{}]: {}\n", source, content));
            if i < raw_results.len() - 1 {
                context_str.push('\n');
            }
        }
        Ok(context_str)
    }
}

// =========================================================================
// TESTS
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::test_mocks::inject_mock_config;
    use crate::utils::io::tempdir;
    use crate::utils::{AsyncMutex, OnceLock};

    fn get_hf_lock() -> &'static AsyncMutex<()> {
        static LOCK: OnceLock<AsyncMutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| AsyncMutex::new(()))
    }

    #[tokio::test]
    async fn test_rag_candle_end_to_end() {
        inject_mock_config();

        let _guard = get_hf_lock().lock().await;
        let dir = tempdir().unwrap();
        let mut rag = RagRetriever::new_internal(dir.path().to_path_buf())
            .await
            .unwrap();

        let content = "Le module de sécurité requiert une validation cryptographique SHA-256.";
        rag.index_document(content, "spec_secu_v2.pdf")
            .await
            .unwrap();

        let context = rag
            .retrieve(
                "Quelle validation cryptographique est requise pour le module de sécurité ?",
                1,
            )
            .await
            .unwrap();
        assert!(context.contains("SHA-256"));
        assert!(context.contains("spec_secu_v2.pdf"));
    }

    #[tokio::test]
    async fn test_rag_candle_empty_results() {
        inject_mock_config();

        let _guard = get_hf_lock().lock().await;
        let dir = tempdir().unwrap();
        let mut rag = RagRetriever::new_internal(dir.path().to_path_buf())
            .await
            .unwrap();

        rag.index_document("Recette de la tarte aux pommes.", "cuisine.txt")
            .await
            .unwrap();
        let context = rag
            .retrieve("Comment configurer le réseau TCP ?", 1)
            .await
            .unwrap();
        assert_eq!(context, "");
    }

    #[tokio::test]
    async fn test_rag_candle_persistence() {
        inject_mock_config();

        let _guard = get_hf_lock().lock().await;
        let dir = tempdir().unwrap();

        {
            let mut rag = RagRetriever::new_internal(dir.path().to_path_buf())
                .await
                .unwrap();
            rag.index_document("La persistance Zstd est hyper rapide.", "doc_io")
                .await
                .unwrap();
        }

        {
            let mut new_rag = RagRetriever::new_internal(dir.path().to_path_buf())
                .await
                .unwrap();
            let context = new_rag
                .retrieve("Est-ce que la persistance Zstd est rapide ?", 1)
                .await
                .unwrap();
            assert!(context.contains("hyper rapide"));
        }
    }

    #[tokio::test]
    async fn test_rag_chunking_logic() {
        inject_mock_config();

        let _guard = get_hf_lock().lock().await;
        let dir = tempdir().unwrap();
        let mut rag = RagRetriever::new_internal(dir.path().to_path_buf())
            .await
            .unwrap();

        let long_text = "Moteur ".repeat(1500);
        let count = rag.index_document(&long_text, "stress_test").await.unwrap();
        assert!(count > 1);
    }
}
