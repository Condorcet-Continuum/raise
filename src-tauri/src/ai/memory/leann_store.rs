use super::{MemoryRecord, VectorStore};
use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

pub struct LeannMemory {
    base_url: String,
    client: reqwest::Client,
}

impl LeannMemory {
    pub fn new(url: &str) -> Result<Self> {
        let clean_url = url.trim_end_matches('/').to_string();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(5))
            .build()?;
        Ok(Self {
            base_url: clean_url,
            client,
        })
    }
}

#[derive(Serialize)]
struct ServerDocument {
    text: String,
    metadata: serde_json::Value,
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
    text: String,
    score: f32,
    metadata: Option<serde_json::Value>,
}
#[derive(Deserialize)]
struct ServerSearchResponse {
    results: Vec<ServerSearchResultItem>,
}

#[async_trait]
impl VectorStore for LeannMemory {
    async fn init_collection(&self, _col: &str, _size: u64) -> Result<()> {
        let url = format!("{}/health", self.base_url);
        let res = self
            .client
            .get(&url)
            .send()
            .await
            .context("Serveur LEANN injoignable")?;
        if !res.status().is_success() {
            anyhow::bail!("LEANN Health Error");
        }
        Ok(())
    }

    async fn add_documents(&self, _col: &str, records: Vec<MemoryRecord>) -> Result<()> {
        let url = format!("{}/insert", self.base_url);
        let server_docs = records
            .iter()
            .map(|r| ServerDocument {
                text: r.content.clone(),
                metadata: r.metadata.clone(),
            })
            .collect();

        let res = self
            .client
            .post(&url)
            .json(&ServerInsertRequest {
                documents: server_docs,
            })
            .send()
            .await?;
        if !res.status().is_success() {
            anyhow::bail!("LEANN Insert Error");
        }
        Ok(())
    }

    async fn search_similarity(
        &self,
        _col: &str,
        _vec: &[f32],
        limit: u64,
        threshold: f32,
        _filter: Option<HashMap<String, String>>,
    ) -> Result<Vec<MemoryRecord>> {
        let url = format!("{}/search", self.base_url);
        let res = self
            .client
            .post(&url)
            .json(&ServerSearchRequest { k: limit })
            .send()
            .await?;
        let response: ServerSearchResponse = res.json().await?;

        Ok(response
            .results
            .into_iter()
            .filter(|r| r.score >= threshold)
            .map(|r| MemoryRecord {
                id: r.id,
                content: r.text,
                metadata: r.metadata.unwrap_or(serde_json::Value::Null),
                vectors: None,
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_leann_client_init() {
        let store = LeannMemory::new("http://localhost:8000");
        assert!(store.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_leann_health_fail() {
        // Test réel contre un port vide, doit échouer
        let store = LeannMemory::new("http://127.0.0.1:9999").unwrap();
        let res = store.init_collection("any", 384).await;
        assert!(res.is_err());
    }
}
