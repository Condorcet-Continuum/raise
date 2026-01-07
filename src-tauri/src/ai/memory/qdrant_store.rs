use super::{MemoryRecord, VectorStore};
use anyhow::{Context, Result};
use async_trait::async_trait;
use qdrant_client::{
    qdrant::{
        point_id::PointIdOptions, vectors_config::Config, CreateCollection, Distance, PointId,
        PointStruct, SearchPoints, UpsertPoints, VectorParams, VectorsConfig, WithPayloadSelector,
    },
    Payload, Qdrant,
};
use serde_json::json;
use uuid::Uuid;

pub struct QdrantMemory {
    client: Qdrant,
}

impl QdrantMemory {
    pub fn new(url: &str) -> Result<Self> {
        let client = Qdrant::from_url(url).build()?;
        Ok(Self { client })
    }
}

/// Helper pour convertir l'ID Qdrant (Enum Protobuf) en String simple
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
            println!("🧠 Création de la collection Qdrant : {}", collection_name);
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
                .await
                .context("Impossible de créer la collection Qdrant")?;
        }
        Ok(())
    }

    async fn add_documents(&self, collection_name: &str, records: Vec<MemoryRecord>) -> Result<()> {
        if records.is_empty() {
            return Ok(());
        }

        let points: Vec<PointStruct> = records
            .into_iter()
            .map(|record| {
                let id = Uuid::parse_str(&record.id).unwrap_or_else(|_| Uuid::new_v4());

                let mut json_meta = record.metadata;
                if let Some(obj) = json_meta.as_object_mut() {
                    obj.insert("content".to_string(), json!(record.content));
                }

                let payload: Payload = json_meta.try_into().unwrap_or_default();
                let vector = record.vectors.unwrap_or_default();

                PointStruct::new(id.to_string(), vector, payload)
            })
            .collect();

        let request = UpsertPoints {
            collection_name: collection_name.to_string(),
            points,
            ..Default::default()
        };

        self.client.upsert_points(request).await?;

        Ok(())
    }

    async fn search_similarity(
        &self,
        collection_name: &str,
        vector: &[f32],
        limit: u64,
        score_threshold: f32,
    ) -> Result<Vec<MemoryRecord>> {
        let search_result = self
            .client
            .search_points(SearchPoints {
                collection_name: collection_name.to_string(),
                vector: vector.to_vec(),
                limit,
                score_threshold: Some(score_threshold),
                with_payload: Some(WithPayloadSelector {
                    selector_options: Some(
                        qdrant_client::qdrant::with_payload_selector::SelectorOptions::Enable(true),
                    ),
                }),
                ..Default::default()
            })
            .await?;

        let results = search_result
            .result
            .into_iter()
            .map(|point| {
                let id_str = point_id_to_string(point.id);

                // CORRECTION ICI : Utilisation de Payload::from() au lieu de new_from_hashmap()
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
            .collect();

        Ok(results)
    }
}
