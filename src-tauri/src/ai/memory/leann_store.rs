use super::{MemoryRecord, VectorStore};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Client pour communiquer avec le serveur Python LEANN via HTTP
pub struct LeannMemory {
    base_url: String,
    client: reqwest::Client,
}

impl LeannMemory {
    pub fn new(url: &str) -> Result<Self> {
        let clean_url = url.trim_end_matches('/').to_string();
        Ok(Self {
            base_url: clean_url,
            client: reqwest::Client::new(),
        })
    }
}

// --- STRUCTURES INTERNES POUR LE SERVEUR ---
// Ces structures matchent exactement ce que le main.rs du serveur attend

#[derive(Serialize)]
struct ServerDocument {
    text: String,
}

#[derive(Serialize)]
struct ServerInsertRequest {
    documents: Vec<ServerDocument>,
}

#[derive(Serialize)]
struct ServerSearchRequest {
    k: u64,
}

#[derive(Deserialize)]
struct ServerSearchResultItem {
    id: String,
    text: String, // Le serveur renvoie 'text', pas 'content'
    score: f32,
}

#[derive(Deserialize)]
struct ServerSearchResponse {
    results: Vec<ServerSearchResultItem>,
}

#[async_trait]
impl VectorStore for LeannMemory {
    // 1. CORRECTION : On utilise /health au lieu de /init
    async fn init_collection(&self, _collection_name: &str, _vector_size: u64) -> Result<()> {
        let url = format!("{}/health", self.base_url);

        let res = self
            .client
            .get(&url)
            .send()
            .await
            .context("Impossible de contacter le serveur LEANN")?;

        if !res.status().is_success() {
            anyhow::bail!("Le serveur LEANN n'est pas prêt: {}", res.status());
        }

        println!("🧠 LEANN : Connexion établie avec succès.");
        Ok(())
    }

    // 2. CORRECTION : On utilise /insert et on mappe MemoryRecord vers ServerDocument
    async fn add_documents(
        &self,
        _collection_name: &str,
        records: Vec<MemoryRecord>,
    ) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let url = format!("{}/insert", self.base_url);

        // Transformation des données pour le serveur
        let server_docs: Vec<ServerDocument> = records
            .iter()
            .map(|r| ServerDocument {
                text: r.content.clone(),
            })
            .collect();

        let payload = ServerInsertRequest {
            documents: server_docs,
        };

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Echec de l'envoi des documents à LEANN")?;

        if !res.status().is_success() {
            let error_text = res.text().await.unwrap_or_default();
            anyhow::bail!("Erreur LEANN insert: {}", error_text);
        }

        Ok(())
    }

    // 3. CORRECTION : On utilise /search et on gère la réponse wrapper { results: [...] }
    async fn search_similarity(
        &self,
        _collection_name: &str,
        _vector: &[f32], // Note: Le serveur actuel calcule lui-même l'embedding, on ignore ce vecteur pour l'instant
        limit: u64,
        score_threshold: f32,
    ) -> Result<Vec<MemoryRecord>> {
        let url = format!("{}/search", self.base_url);

        let payload = ServerSearchRequest { k: limit };

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Echec de la recherche LEANN")?;

        if !res.status().is_success() {
            anyhow::bail!("Erreur recherche LEANN: {}", res.status());
        }

        // On désérialise la réponse englobante
        let response_wrapper: ServerSearchResponse = res.json().await?;

        // Conversion des résultats serveur vers MemoryRecord
        let records = response_wrapper
            .results
            .into_iter()
            .filter(|r| r.score >= score_threshold) // Le score du serveur peut être une distance ou similarité
            .map(|r| MemoryRecord {
                id: r.id,
                content: r.text,                   // Mapping 'text' -> 'content'
                metadata: serde_json::Value::Null, // Le serveur actuel ne stocke pas encore les métadonnées
                vectors: None,
            })
            .collect();

        Ok(records)
    }
}
