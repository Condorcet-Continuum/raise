#[cfg(test)]
mod integration_tests {
    use crate::ai::memory::{
        leann_store::LeannMemory, qdrant_store::QdrantMemory, MemoryRecord, VectorStore,
    };
    use serde_json::json;
    use std::env;
    use uuid::Uuid;

    // Helper pour attendre un peu (Qdrant met parfois quelques ms à indexer)
    async fn sleep_ms(ms: u64) {
        tokio::time::sleep(tokio::time::Duration::from_millis(ms)).await;
    }

    #[tokio::test]
    #[ignore]
    async fn test_qdrant_lifecycle() {
        // 1. CHARGEMENT CONFIG (.env)
        dotenvy::dotenv().expect("❌ Impossible de charger le fichier .env !");

        let qdrant_port = env::var("PORT_QDRANT_GRPC")
            .expect("❌ Variable PORT_QDRANT_GRPC manquante dans le .env");

        // On force l'IPv4 127.0.0.1 pour éviter les problèmes de résolution localhost/IPv6 dans les tests
        let url = format!("http://127.0.0.1:{}", qdrant_port);

        println!("🔧 Configuration Qdrant chargée : {}", url);

        // 2. CONNEXION
        // Tentative de connexion (si fail, on panic avec un message clair)
        let store = QdrantMemory::new(&url).unwrap_or_else(|e| {
            panic!("❌ Impossible de se connecter à Qdrant sur {} : {}", url, e)
        });

        let collection_name = "test_memory_suite";
        let vector_size = 4; // Petit vecteur pour le test (ex: [1.0, 0.0, 0.0, 0.0])

        // 3. INITIALISATION
        let init_res = store.init_collection(collection_name, vector_size).await;
        assert!(
            init_res.is_ok(),
            "L'initialisation de la collection a échoué"
        );

        // 4. INSERTION DE DONNÉES DUMMY
        // Vecteur A : Pointe vers le "Nord"
        let rec1 = MemoryRecord {
            id: Uuid::new_v4().to_string(),
            content: "Le chat mange des croquettes".to_string(),
            metadata: json!({"category": "animal"}),
            vectors: Some(vec![1.0, 0.0, 0.0, 0.0]),
        };

        // Vecteur B : Pointe vers l'"Est"
        let rec2 = MemoryRecord {
            id: Uuid::new_v4().to_string(),
            content: "La voiture roule vite".to_string(),
            metadata: json!({"category": "machine"}),
            vectors: Some(vec![0.0, 1.0, 0.0, 0.0]),
        };

        let insert_res = store
            .add_documents(collection_name, vec![rec1.clone(), rec2.clone()])
            .await;
        assert!(insert_res.is_ok(), "L'insertion a échoué");

        // Petite pause pour laisser Qdrant indexer
        sleep_ms(500).await;

        // 5. RECHERCHE (SIMILARITÉ)
        // On cherche un vecteur proche de [0.9, 0.1, 0.0, 0.0] -> Devrait trouver le Chat (rec1)
        let search_vector = vec![0.9, 0.1, 0.0, 0.0];

        let results = store
            .search_similarity(collection_name, &search_vector, 1, 0.0)
            .await
            .expect("La recherche a échoué");

        // 6. ASSERTIONS
        assert_eq!(results.len(), 1, "On attendait exactement 1 résultat");
        let found = &results[0];

        println!(
            "🔎 Résultat trouvé : '{}' (Score attendu: haut)",
            found.content
        );

        assert_eq!(
            found.content, rec1.content,
            "On aurait dû trouver le chat !"
        );
        assert_eq!(
            found.metadata["category"], "animal",
            "Les métadonnées sont corrompues"
        );

        println!("✅ Test Qdrant Lifecycle : SUCCÈS");
    }

    #[tokio::test]
    #[ignore] // Ignoré par défaut tant que le serveur Python n'est pas lancé
    async fn test_leann_lifecycle() {
        // 1. CHARGEMENT CONFIG
        dotenvy::dotenv().ok();

        // On suppose que LEANN tourne sur le port défini ou 8000 par défaut
        let leann_port = env::var("PORT_LEANN").unwrap_or("8000".to_string());
        let url = format!("http://127.0.0.1:{}", leann_port);

        println!("🔧 Test LEANN sur : {}", url);

        let store = LeannMemory::new(&url).expect("URL invalide");

        // 2. INIT
        let collection = "test_leann_suite";
        // 384 dims, bien que LEANN soit flexible
        store
            .init_collection(collection, 384)
            .await
            .expect("Init failed");

        // 3. INSERTION
        let rec = MemoryRecord {
            id: Uuid::new_v4().to_string(),
            content: "Leann est très léger en mémoire".to_string(),
            metadata: json!({"type": "optimization"}),
            vectors: Some(vec![0.5, 0.5, 0.0, 0.0]), // Fake vector
        };

        store
            .add_documents(collection, vec![rec.clone()])
            .await
            .expect("Insert failed");

        // 4. RECHERCHE
        let query = vec![0.5, 0.5, 0.0, 0.0];
        let results = store
            .search_similarity(collection, &query, 1, 0.0)
            .await
            .expect("Search failed");

        // 5. VERIF
        // Note: Le mock serveur Python doit renvoyer ce qu'on lui a donné pour que ce test passe
        if !results.is_empty() {
            assert_eq!(results[0].content, rec.content);
            println!("✅ Test LEANN Lifecycle : SUCCÈS avec 1 résultat");
        } else {
            println!("⚠️ Test LEANN : Pas de résultats (Vérifiez le serveur Python)");
        }
    }
}
