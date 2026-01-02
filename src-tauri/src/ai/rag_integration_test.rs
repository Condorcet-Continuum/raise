#[cfg(test)]
mod tests {
    use crate::ai::memory::{qdrant_store::QdrantMemory, MemoryRecord, VectorStore};
    use crate::ai::nlp::embeddings::EmbeddingEngine;
    use serde_json::json;
    use std::env;
    use uuid::Uuid;

    #[tokio::test]
    #[ignore]
    async fn test_full_rag_pipeline() {
        println!("🚀 Démarrage du test RAG complet...");

        // 1. CHARGEMENT DE LA CONFIGURATION (.env)
        dotenvy::dotenv().expect("❌ Impossible de charger le fichier .env !");

        let qdrant_port = env::var("PORT_QDRANT_GRPC")
            .expect("❌ Variable PORT_QDRANT_GRPC manquante dans le .env");

        // On force 127.0.0.1 pour la stabilité Docker/Rust (évite les soucis IPv6 localhost)
        let qdrant_url = format!("http://127.0.0.1:{}", qdrant_port);

        println!("🔧 Configuration Qdrant chargée : {}", qdrant_url);

        // 2. INITIALISATION
        // Connexion avec l'URL dynamique
        let memory = QdrantMemory::new(&qdrant_url).unwrap_or_else(|e| {
            panic!(
                "❌ Impossible de se connecter à Qdrant sur {} : {}",
                qdrant_url, e
            )
        });

        let mut embedder = EmbeddingEngine::new()
            .expect("❌ Impossible de charger le modèle d'embedding (FastEmbed)");

        let collection = "raise_integration_test";

        memory
            .init_collection(collection, 384)
            .await
            .expect("❌ Échec initialisation collection");

        // 3. INGESTION DE DONNÉES DE TEST
        let knowledge_base = vec![
            "La batterie du drone a une capacité de 5000mAh.",
            "Le protocole de communication est chiffré en AES-256.",
            "La réunion de projet est prévue lundi matin.",
        ];

        println!("🧠 Vectorisation de {} documents...", knowledge_base.len());
        let vectors = embedder
            .embed_batch(knowledge_base.iter().map(|s| s.to_string()).collect())
            .expect("❌ Échec Embedding");

        let records: Vec<MemoryRecord> = knowledge_base
            .iter()
            .zip(vectors.into_iter())
            .map(|(text, vector)| MemoryRecord {
                id: Uuid::new_v4().to_string(),
                content: text.to_string(),
                metadata: json!({"source": "manual_test"}),
                vectors: Some(vector),
            })
            .collect();

        memory
            .add_documents(collection, records)
            .await
            .expect("❌ Échec Stockage Qdrant");

        // Petite pause pour laisser le temps à Qdrant d'indexer
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // 4. RECHERCHE SÉMANTIQUE
        let query = "Quelle est l'autonomie électrique ?";
        println!("❓ Question : '{}'", query);

        let query_vector = embedder
            .embed_query(query)
            .expect("❌ Échec Embedding Query");

        let results = memory
            .search_similarity(collection, &query_vector, 1, 0.4)
            .await
            .expect("❌ Échec Recherche");

        assert!(!results.is_empty(), "Aucun résultat trouvé !");

        let best_match = &results[0];
        println!("💡 Meilleur résultat : '{}'", best_match.content);

        // 5. VALIDATION SÉMANTIQUE
        assert!(
            best_match.content.contains("batterie") || best_match.content.contains("5000mAh"),
            "❌ Mauvaise réponse sémantique trouvée"
        );

        println!("✅ SUCCÈS RAG COMPLET !");
    }
}
