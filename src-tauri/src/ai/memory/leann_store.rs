use super::{MemoryRecord, VectorStore};
use crate::utils::{
    async_trait,
    net_client::{get_simple, post_json_with_retry},
    prelude::*,
    HashMap,
};

pub struct LeannMemory {
    base_url: String,
    // 🗑️ PLUS DE `client: reqwest::Client` ! Le Singleton de la façade s'en charge.
}

impl LeannMemory {
    pub fn new(url: &str) -> Result<Self> {
        let clean_url = url.trim_end_matches('/').to_string();
        Ok(Self {
            base_url: clean_url,
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

        // ✅ Utilisation de la façade (gère le Timeout, les statuts et l'erreur)
        get_simple(&url)
            .await
            .map_err(|e| AppError::from(format!("LEANN Health Error: {}", e)))?;

        Ok(())
    }

    async fn add_documents(&self, _col: &str, records: Vec<MemoryRecord>) -> Result<()> {
        let url = format!("{}/insert", self.base_url);
        let server_docs = records
            .into_iter()
            .map(|r| ServerDocument {
                text: r.content,
                metadata: r.metadata,
            })
            .collect();

        let request = ServerInsertRequest {
            documents: server_docs,
        };

        // ✅ Utilisation de la façade pour le POST JSON (AppError est géré en interne !)
        let _res: Value = post_json_with_retry(&url, &request, 1).await?;

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
        let request = ServerSearchRequest { k: limit };

        // ✅ Utilisation de la façade avec la magie de Serde pour parser la réponse
        let response: ServerSearchResponse = post_json_with_retry(&url, &request, 1).await?;

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
        // Test réel contre un port vide, doit échouer proprement via la façade
        let store = LeannMemory::new("http://127.0.0.1:9999").unwrap();
        let res = store.init_collection("any", 384).await;

        assert!(res.is_err(), "Devrait renvoyer une erreur réseau");
    }
}
