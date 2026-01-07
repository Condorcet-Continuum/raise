use super::{MemoryRecord, VectorStore};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Client pour communiquer avec le serveur Python LEANN via HTTP
pub struct LeannMemory {
    base_url: String,
    client: reqwest::Client,
}

impl LeannMemory {
    pub fn new(url: &str) -> Result<Self> {
        // On s'assure que l'URL ne finit pas par '/' pour la concaténation propre
        let clean_url = url.trim_end_matches('/').to_string();
        Ok(Self {
            base_url: clean_url,
            client: reqwest::Client::new(),
        })
    }
}

// Structures pour la communication JSON avec le serveur Python
#[derive(Serialize)]
struct LeannSearchRequest {
    collection: String,
    vector: Vec<f32>,
    limit: u64,
}

#[derive(Deserialize)]
struct LeannSearchResult {
    id: String,
    content: String,
    metadata: serde_json::Value,
    score: f32,
}

#[async_trait]
impl VectorStore for LeannMemory {
    async fn init_collection(&self, collection_name: &str, _vector_size: u64) -> Result<()> {
        // LEANN gère l'index dynamiquement, mais on peut pinguer le serveur pour dire "prépare ce dossier"
        let url = format!("{}/init", self.base_url);

        let payload = json!({
            "collection": collection_name
        });

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Impossible de contacter le serveur LEANN")?;

        if !res.status().is_success() {
            anyhow::bail!("Erreur serveur LEANN lors de l'init: {}", res.status());
        }

        println!(
            "🧠 LEANN : Collection '{}' prête (virtuellement)",
            collection_name
        );
        Ok(())
    }

    async fn add_documents(&self, collection_name: &str, records: Vec<MemoryRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let url = format!("{}/upsert", self.base_url);

        // On envoie le batch complet
        let payload = json!({
            "collection": collection_name,
            "documents": records
        });

        let res = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .context("Echec de l'envoi des documents à LEANN")?;

        if !res.status().is_success() {
            let error_text = res.text().await.unwrap_or_default();
            anyhow::bail!("Erreur LEANN upsert: {}", error_text);
        }

        Ok(())
    }

    async fn search_similarity(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
        score_threshold: f32,
    ) -> Result<Vec<MemoryRecord>> {
        let url = format!("{}/search", self.base_url);

        let payload = LeannSearchRequest {
            collection: collection_name.to_string(),
            vector: vector.to_vec(),
            limit,
        };

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

        let results: Vec<LeannSearchResult> = res.json().await?;

        // Conversion des résultats bruts en MemoryRecord, en filtrant par score
        let records = results
            .into_iter()
            .filter(|r| r.score >= score_threshold)
            .map(|r| MemoryRecord {
                id: r.id,
                content: r.content,
                metadata: r.metadata,
                vectors: None, // LEANN ne renvoie pas forcément le vecteur source
            })
            .collect();

        Ok(records)
    }
}
