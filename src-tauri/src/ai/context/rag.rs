// FICHIER : src-tauri/src/ai/context/rag.rs
use crate::ai::memory::{candle_store::CandleLocalStore, MemoryRecord, VectorStore};
use crate::ai::nlp::{embeddings::EmbeddingEngine, splitting};
use crate::json_db::collections::manager::CollectionsManager;

use crate::utils::prelude::*;
use candle_core::Device;

pub struct RagRetriever {
    backend: CandleLocalStore, // 🎯 Connexion directe et exclusive au moteur natif
    embedder: EmbeddingEngine,
    collection_name: String,
}

impl RagRetriever {
    /// Initialise le RAG en se basant EXCLUSIVEMENT sur la configuration globale
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let config = AppConfig::get();
        let storage_path = config
            .get_path("PATH_RAISE_DOMAIN")
            .unwrap_or_else(|| PathBuf::from("./raise_default_domain"));

        Self::new_internal(storage_path, manager).await
    }

    /// Constructeur interne pour injecter le path et le manager (Idéal pour les tests)
    pub async fn new_internal(
        storage_path: PathBuf,
        manager: &CollectionsManager<'_>,
    ) -> RaiseResult<Self> {
        // 🎯 L'injection de dépendance avec .await est ici !
        let embedder = EmbeddingEngine::new(manager).await?;

        let collection_name = "raise_knowledge_base".to_string();

        user_info!(
            "INF_RAG_CANDLE_INIT",
            json_value!({"backend": "CANDLE", "device": "Native"})
        );

        let device = Device::Cpu;
        let store_dir = storage_path.join("vector_store");
        let memory = CandleLocalStore::new(&store_dir, &device);

        // 🎯 FIX : On passe le manager à init_collection
        memory
            .init_collection(manager, &collection_name, 384)
            .await?;
        memory.load().await?; // Charge l'historique s'il existe

        Ok(Self {
            backend: memory,
            embedder,
            collection_name,
        })
    }

    pub async fn index_document(
        &mut self,
        manager: &CollectionsManager<'_>, // 🎯 FIX : Ajout du manager
        content: &str,
        source: &str,
    ) -> RaiseResult<usize> {
        let chunks = splitting::split_text_into_chunks(content, 512);
        if chunks.is_empty() {
            return Ok(0);
        }

        let vectors = self.embedder.embed_batch(chunks.clone())?;
        let ingest_time = UtcClock::now().to_rfc3339();

        let mut records = Vec::new();
        for (i, chunk) in chunks.iter().enumerate() {
            records.push(MemoryRecord {
                id: UniqueId::new_v4().to_string(),
                content: chunk.clone(),
                metadata: json_value!({
                    "source": source,
                    "chunk_index": i,
                    "total_chunks": chunks.len(),
                    "ingested_at": ingest_time
                }),
                vectors: Some(vectors[i].clone()),
            });
        }

        // 🎯 FIX : On passe le manager à add_documents
        self.backend
            .add_documents(manager, &self.collection_name, records)
            .await?;
        self.backend.save().await?; // Persistance immédiate

        Ok(chunks.len())
    }

    pub async fn retrieve(
        &mut self,
        manager: &CollectionsManager<'_>, // 🎯 FIX : Ajout du manager
        query: &str,
        limit: u64,
    ) -> RaiseResult<String> {
        let query_vector = self.embedder.embed_query(query)?;

        // Seuil ajusté pour le modèle multilingue
        let min_similarity = 0.65;

        // 🎯 FIX : On passe le manager à search_similarity
        let docs = self
            .backend
            .search_similarity(
                manager,
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
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    fn get_hf_lock() -> &'static AsyncMutex<()> {
        static LOCK: StaticCell<AsyncMutex<()>> = StaticCell::new();
        LOCK.get_or_init(|| AsyncMutex::new(()))
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_rag_candle_end_to_end() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(&manager, "nlp",  json_value!({ "model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors" })).await;

        let mut rag = RagRetriever::new_internal(sandbox.domain_root.clone(), &manager)
            .await
            .unwrap();

        let content = "Le module de sécurité requiert une validation cryptographique SHA-256.";

        // 🎯 FIX : Passage de &manager
        rag.index_document(&manager, content, "spec_secu_v2.pdf")
            .await
            .unwrap();

        // 🎯 FIX : Passage de &manager
        let context = rag
            .retrieve(
                &manager,
                "Quelle validation cryptographique est requise pour le module de sécurité ?",
                1,
            )
            .await
            .unwrap();
        assert!(context.contains("SHA-256"));
        assert!(context.contains("spec_secu_v2.pdf"));
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_rag_candle_empty_results() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(&manager, "nlp", crate::utils::json::json_value!({ "model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors" })).await;

        let mut rag = RagRetriever::new_internal(sandbox.domain_root.clone(), &manager)
            .await
            .unwrap();

        // 🎯 FIX : Passage de &manager
        rag.index_document(&manager, "Recette de la tarte aux pommes.", "cuisine.txt")
            .await
            .unwrap();

        // 🎯 FIX : Passage de &manager
        let context = rag
            .retrieve(&manager, "Comment configurer le réseau TCP ?", 1)
            .await
            .unwrap();
        assert_eq!(context, "");
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_rag_candle_persistence() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        inject_mock_component(&manager, "nlp",  json_value!({ "model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors" })).await;

        {
            let mut rag = RagRetriever::new_internal(sandbox.domain_root.clone(), &manager)
                .await
                .unwrap();

            // 🎯 FIX : Passage de &manager
            rag.index_document(&manager, "La persistance Zstd est hyper rapide.", "doc_io")
                .await
                .unwrap();
        }

        {
            let mut new_rag = RagRetriever::new_internal(sandbox.domain_root.clone(), &manager)
                .await
                .unwrap();

            // 🎯 FIX : Passage de &manager
            let context = new_rag
                .retrieve(&manager, "Est-ce que la persistance Zstd est rapide ?", 1)
                .await
                .unwrap();
            assert!(context.contains("hyper rapide"));
        }
    }

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_rag_chunking_logic() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        inject_mock_component(&manager, "nlp", crate::utils::json::json_value!({ "model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors" })).await;

        let mut rag = RagRetriever::new_internal(sandbox.domain_root.clone(), &manager)
            .await
            .unwrap();

        let long_text = "Moteur ".repeat(1500);

        // 🎯 FIX : Passage de &manager
        let count = rag
            .index_document(&manager, &long_text, "stress_test")
            .await
            .unwrap();
        assert!(count > 1);
    }
}
