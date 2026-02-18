use crate::utils::{io::PathBuf, prelude::*, Uuid};

use crate::ai::memory::{qdrant_store::QdrantMemory, MemoryRecord, VectorStore};
use crate::ai::nlp::{embeddings::EmbeddingEngine, splitting};
use crate::graph_store::GraphStore;

enum RagBackend {
    Qdrant(QdrantMemory),
    Surreal(GraphStore),
}

pub struct RagRetriever {
    backend: RagBackend,
    embedder: EmbeddingEngine,
    collection_name: String,
}

impl RagRetriever {
    pub async fn new(qdrant_url: &str, storage_path: PathBuf) -> Result<Self> {
        let embedder = EmbeddingEngine::new()
            .map_err(|e| AppError::Ai(format!("Échec init Embedder: {}", e)))?;
        let collection_name = "raise_knowledge_base".to_string();

        let provider = AppConfig::get().core.vector_store_provider.clone();
        println!(
            "📚 [RAG] Initialisation du backend : {}",
            provider.to_uppercase()
        );

        let backend = match provider.as_str() {
            "qdrant" => {
                let memory = QdrantMemory::new(qdrant_url)?;
                memory.init_collection(&collection_name, 384).await?;
                RagBackend::Qdrant(memory)
            }
            _ => {
                let store = GraphStore::new(storage_path).await?;
                RagBackend::Surreal(store)
            }
        };

        Ok(Self {
            backend,
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

        match &self.backend {
            RagBackend::Qdrant(memory) => {
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
                memory.add_documents(&self.collection_name, records).await?;
            }
            RagBackend::Surreal(store) => {
                for (i, chunk) in chunks.iter().enumerate() {
                    let id = Uuid::new_v4().to_string();
                    let data = json!({
                        "content": chunk,
                        "source": source,
                        "chunk_index": i,
                        "ingested_at": ingest_time,
                        "embedding": vectors[i]
                    });
                    store.index_entity(&self.collection_name, &id, data).await?;
                }
            }
        }
        Ok(chunks.len())
    }

    pub async fn retrieve(&mut self, query: &str, limit: u64) -> Result<String> {
        let query_vector = self.embedder.embed_query(query)?;

        let raw_results: Vec<(String, String)> = match &self.backend {
            RagBackend::Qdrant(memory) => {
                let docs = memory
                    .search_similarity(&self.collection_name, &query_vector, limit, 0.4, None)
                    .await?;
                docs.into_iter()
                    .map(|d| {
                        let src = d
                            .metadata
                            .get("source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        (src, d.content)
                    })
                    .collect()
            }
            RagBackend::Surreal(store) => {
                // Utilisation directe du backend pour bypasser la logique conditionnelle du GraphStore
                let docs = store
                    .backend()
                    .search_similar(&self.collection_name, query_vector, limit as usize)
                    .await?;

                // DIAGNOSTIC EN CAS D'ÉCHEC
                if docs.is_empty() {
                    let count_sql = format!(
                        "SELECT count() FROM type::table('{}');",
                        self.collection_name
                    );
                    let debug_res = store
                        .backend()
                        .raw_query(&count_sql)
                        .await
                        .unwrap_or_default();
                    println!(
                        "⚠️ [RAG DEBUG] Aucune similarité trouvée. Documents dans la DB : {:?}",
                        debug_res
                    );
                }

                docs.into_iter()
                    .map(|d| {
                        let src = d
                            .get("source")
                            .and_then(|v| v.as_str())
                            .unwrap_or("?")
                            .to_string();
                        let content = d
                            .get("content")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        (src, content)
                    })
                    .collect()
            }
        };

        if raw_results.is_empty() {
            return Ok(String::new());
        }

        let mut context_str = String::from("### DOCUMENTATION PERTINENTE (RAG) ###\n");
        for (i, (source, content)) in raw_results.iter().enumerate() {
            context_str.push_str(&format!("Source [{}]: {}\n", source, content));
            if i < raw_results.len() - 1 {
                context_str.push_str("---\n");
            }
        }
        Ok(context_str)
    }
}

// --- TESTS ---
#[cfg(test)]
mod tests {
    use super::*;
    // ✅ On importe les outils de la façade utils
    use crate::utils::io::tempdir;

    // ✅ On utilise Once pour garantir que la config n'est initialisée qu'une seule fois
    // même si les tests s'exécutent en parallèle (multithreading).
    /*
    static INIT_TEST: Once = Once::new();

    fn setup_test_env() {
        INIT_TEST.call_once(|| {
            // Initialise la configuration globale pour les tests
            if let Err(e) = AppConfig::init() {
                eprintln!(
                    "⚠️ Info: Configuration déjà initialisée ou impossible à charger: {}",
                    e
                );
            }
        });
    }
    */

    #[tokio::test]
    async fn test_rag_backend_surreal_default() {
        // ✅ On utilise l'injecteur de Mock mémoire au lieu de setup_test_env()
        crate::utils::config::test_mocks::inject_mock_config();

        let dir = tempdir().unwrap();
        let mut rag = RagRetriever::new("http://dummy", dir.path().to_path_buf())
            .await
            .expect("Init Surreal RAG Failed");

        let content = "Le système RAISE utilise une architecture hybride Rust/React.";
        let count = rag
            .index_document(content, "doc_tech_v1")
            .await
            .expect("Index failed");
        assert_eq!(count, 1, "Texte court = 1 chunk");

        // On attend un peu pour être sûr que Surreal a commité (bien que ce soit synchrone en local)
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        let context = rag
            .retrieve("Quelle est l'architecture ?", 1)
            .await
            .expect("Search failed");

        println!("Context Found: {}", context);
        assert!(context.contains("RAISE"));
        assert!(context.contains("Rust/React"));
    }

    #[tokio::test]
    async fn test_rag_chunking_logic() {
        // ✅ Remplace l'ancien `EnvReset`
        crate::utils::config::test_mocks::inject_mock_config();

        let dir = tempdir().unwrap();
        let mut rag = RagRetriever::new("http://dummy", dir.path().to_path_buf())
            .await
            .unwrap();

        let long_text = "Word ".repeat(1000);
        let count = rag.index_document(&long_text, "stress_test").await.unwrap();
        assert!(count > 1);
    }
}
