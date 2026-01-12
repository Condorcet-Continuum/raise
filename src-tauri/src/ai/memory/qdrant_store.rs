use super::{MemoryRecord, VectorStore};
use anyhow::Result;
use async_trait::async_trait;
use qdrant_client::{
    qdrant::{
        point_id::PointIdOptions, vectors_config::Config, Condition, CreateCollection, Distance,
        Filter, PointId, PointStruct, SearchPoints, UpsertPoints, VectorParams, VectorsConfig,
        WithPayloadSelector,
    },
    Payload, Qdrant,
};
use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

pub struct QdrantMemory {
    client: Qdrant,
}

impl QdrantMemory {
    pub fn new(url: &str) -> Result<Self> {
        // 1. On crée d'abord l'objet de configuration
        let mut config = qdrant_client::Qdrant::from_url(url);

        // 2. On modifie le champ directement (c'est un booléen)
        config.check_compatibility = false;

        // 3. On construit le client avec cette configuration modifiée
        let client = config.build()?;

        Ok(Self { client })
    }
}

fn point_id_to_string(point_id: Option<PointId>) -> String {
    match point_id {
        Some(PointId {
            point_id_options: Some(opts),
        }) => match opts {
            PointIdOptions::Num(n) => n.to_string(),
            PointIdOptions::Uuid(u) => u,
        },
        _ => "unknown".to_string(),
    }
}

#[async_trait]
impl VectorStore for QdrantMemory {
    async fn init_collection(&self, collection_name: &str, vector_size: u64) -> Result<()> {
        if !self.client.collection_exists(collection_name).await? {
            self.client
                .create_collection(CreateCollection {
                    collection_name: collection_name.to_string(),
                    vectors_config: Some(VectorsConfig {
                        config: Some(Config::Params(VectorParams {
                            size: vector_size,
                            distance: Distance::Cosine.into(),
                            ..Default::default()
                        })),
                    }),
                    ..Default::default()
                })
                .await?;
        }
        Ok(())
    }

    async fn add_documents(&self, collection_name: &str, records: Vec<MemoryRecord>) -> Result<()> {
        let mut points = Vec::new();
        for record in records {
            let id = Uuid::parse_str(&record.id).unwrap_or_else(|_| Uuid::new_v4());
            let mut json_meta = record.metadata.clone();
            if let Some(obj) = json_meta.as_object_mut() {
                obj.insert("content".to_string(), json!(record.content));
            }
            let payload: Payload = json_meta.try_into().unwrap_or_default();
            points.push(PointStruct::new(
                id.to_string(),
                record.vectors.unwrap_or_default(),
                payload,
            ));
        }
        self.client
            .upsert_points(UpsertPoints {
                collection_name: collection_name.to_string(),
                points,
                ..Default::default()
            })
            .await?;
        Ok(())
    }

    async fn search_similarity(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
        score_threshold: f32,
        filter_map: Option<HashMap<String, String>>,
    ) -> Result<Vec<MemoryRecord>> {
        let mut filter = None;
        if let Some(f_map) = filter_map {
            let mut conditions = Vec::new();
            for (key, val) in f_map {
                conditions.push(Condition::matches(key, val));
            }
            filter = Some(Filter::all(conditions));
        }

        let search_result = self
            .client
            .search_points(SearchPoints {
                collection_name: collection_name.to_string(),
                vector: vector.to_vec(),
                limit,
                score_threshold: Some(score_threshold),
                filter,
                with_payload: Some(WithPayloadSelector {
                    selector_options: Some(
                        qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
                    ),
                }),
                ..Default::default()
            })
            .await?;

        Ok(search_result
            .result
            .into_iter()
            .map(|point| {
                let id_str = point_id_to_string(point.id);
                // Conversion robuste du Payload Qdrant vers JSON
                let payload_struct = Payload::from(point.payload);
                let json_meta: serde_json::Value = payload_struct.into();

                let content = json_meta
                    .get("content")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                MemoryRecord {
                    id: id_str,
                    content,
                    metadata: json_meta,
                    vectors: None,
                }
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_qdrant_new_client() {
        let store = QdrantMemory::new("http://127.0.0.1:6334");
        assert!(store.is_ok());
    }

    #[tokio::test]
    #[ignore]
    async fn test_qdrant_connection_error() {
        // Ce test doit échouer si Qdrant n'est pas là (quand on le force avec --ignored)
        let store = QdrantMemory::new("http://127.0.0.1:1111").unwrap();
        let res = store.init_collection("test", 4).await;
        assert!(res.is_err(), "Devrait échouer sur un mauvais port");
    }
}
