// FICHIER : src-tauri/src/ai/graph_store/store.rs
use crate::ai::memory::candle_store::CandleLocalStore;
use crate::ai::memory::{MemoryRecord, VectorStore};
use crate::ai::nlp::embeddings::EmbeddingEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::JsonLdProcessor;
use crate::utils::prelude::*;

#[derive(Clone)]
pub struct GraphStore {
    pub storage_path: PathBuf,
    pub vector_store: Option<SharedRef<CandleLocalStore>>,
    pub embedder: Option<SharedRef<AsyncMutex<EmbeddingEngine>>>,
    pub embedding_dim: usize,
    pub processor: JsonLdProcessor,
}

impl GraphStore {
    /// Initialise le GraphStore de manière asynchrone et neutre.
    pub async fn new(storage_path: PathBuf, manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let app_config = AppConfig::get();
        let use_vectors =
            app_config.core.graph_mode == "internal" || app_config.core.graph_mode == "db";
        let embedding_dim = app_config.world_model.embedding_dim;

        let mut vector_store = None;
        let mut embedder = None;

        if use_vectors {
            user_info!("🕸️ [GraphStore] Vectorisation Native activée (JSON-LD + Candle)");

            match EmbeddingEngine::new(manager).await {
                Ok(engine) => {
                    let device = AppConfig::device().clone();
                    let v_dir = storage_path.join("vectors");
                    let v_store = CandleLocalStore::new(&v_dir, &device);

                    // Chargement sécurisé du store existant
                    let _ = v_store.load().await;

                    embedder = Some(SharedRef::new(AsyncMutex::new(engine)));

                    let shared_v_store: SharedRef<CandleLocalStore> = SharedRef::new(v_store);
                    vector_store = Some(shared_v_store);
                }
                Err(e) => {
                    user_warn!(
                        "WRN_GRAPH_STORE_INIT",
                        json_value!({ "error": e.to_string(), "hint": "Mode dégradé sans vecteurs." })
                    );
                }
            }
        }

        Ok(Self {
            storage_path,
            vector_store,
            embedder,
            embedding_dim,
            processor: JsonLdProcessor::new(),
        })
    }

    /// Indexe une entité Arcadia avec normalisation JSON-LD et vectorisation sémantique profonde.
    pub async fn index_entity(
        &self,
        collection: &str,
        id: &str,
        mut data: JsonValue,
    ) -> RaiseResult<()> {
        // 1. Normalisation JSON-LD de l'ID
        if self.processor.get_id(&data).is_none() {
            data["@id"] = json_value!(format!("{}:{}", collection, id));
        }

        // 🎯 OPTIMISATION PROD : Extraction Sémantique Complète
        let text_to_embed = extract_rich_semantic_content(&data);

        // 2. Branche Vectorielle (Inférence + Persistance)
        if let (Some(emb_mutex), Some(v_store)) = (&self.embedder, &self.vector_store) {
            if !text_to_embed.is_empty() {
                let mut engine = emb_mutex.lock().await;
                if let Ok(vector) = engine.embed_query(&text_to_embed) {
                    data["embedding"] = json_value!(vector.clone());

                    let record = MemoryRecord {
                        id: id.to_string(),
                        content: text_to_embed,
                        metadata: json_value!({ "collection": collection }),
                        vectors: Some(vector),
                    };

                    let _ = v_store.add_documents(collection, vec![record]).await;
                    let _ = v_store.save().await;
                }
            }
        }

        // 3. Branche Documentaire (Persistence Physique sur Disque)
        let col_dir = self.storage_path.join("collections").join(collection);
        fs::ensure_dir_async(&col_dir).await?;

        let file_path = col_dir.join(format!("{}.json", id));
        let json_str = json::serialize_to_string_pretty(&data)?;
        fs::write_async(&file_path, json_str).await?;

        Ok(())
    }

    /// Établit un lien sémantique typé entre deux composants Arcadia.
    pub async fn link_entities(
        &self,
        from: (&str, &str),
        relation: &str,
        to: (&str, &str),
    ) -> RaiseResult<()> {
        let (from_col, from_id) = from;
        let file_path = self
            .storage_path
            .join("collections")
            .join(from_col)
            .join(format!("{}.json", from_id));
        let content = fs::read_to_string_async(&file_path).await?;
        let mut doc: JsonValue = json::deserialize_from_str(&content)?;

        let target_uri = format!("{}:{}", to.0, to.1);
        if let Some(obj) = doc.as_object_mut() {
            let rel_array = obj.entry(relation.to_string()).or_insert(json_value!([]));
            if let Some(arr) = rel_array.as_array_mut() {
                let link = json_value!({ "@id": target_uri });
                if !arr.contains(&link) {
                    arr.push(link);
                }
            }
        }

        fs::write_async(&file_path, json::serialize_to_string_pretty(&doc)?).await?;
        Ok(())
    }
}

/// 🎯 OPTIMISATION PROD : Construit une représentation textuelle riche du composant
/// pour maximiser la pertinence des embeddings vectoriels.
fn extract_rich_semantic_content(data: &JsonValue) -> String {
    let mut parts = Vec::new();

    if let Some(obj) = data.as_object() {
        // Priorité 1 : Nom et Description explicites
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            parts.push(format!("Name: {}", name));
        }
        if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
            parts.push(format!("Description: {}", desc));
        }

        // Priorité 2 : Capture de toutes les autres propriétés fonctionnelles
        for (key, value) in obj {
            // On ignore les clés techniques de la DB et du JSON-LD (commençant par _ ou @)
            if key.starts_with('_')
                || key.starts_with('@')
                || key == "name"
                || key == "description"
                || key == "embedding"
            {
                continue;
            }

            if let Some(s) = value.as_str() {
                parts.push(format!("{}: {}", key, s));
            } else if let Some(n) = value.as_f64() {
                parts.push(format!("{}: {}", key, n));
            } else if let Some(b) = value.as_bool() {
                parts.push(format!("{}: {}", key, b));
            } else if let Some(arr) = value.as_array() {
                // Si c'est un tableau de relations Arcadia, on extrait les cibles
                let refs: Vec<String> = arr
                    .iter()
                    .filter_map(|item| {
                        item.get("@id")
                            .and_then(|id| id.as_str())
                            .map(|s| s.to_string())
                    })
                    .collect();

                if !refs.is_empty() {
                    parts.push(format!("{}: [{}]", key, refs.join(", ")));
                }
            }
        }
    }

    parts.join(" | ")
}

// =========================================================================
// TESTS UNITAIRES (LOGIQUE MBSE & PERSISTENCE)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[test]
    fn test_rich_semantic_extraction() {
        let doc = json_value!({
            "@id": "la:Radar",
            "_created_at": "2024-01-01",
            "name": "Radar Module",
            "description": "Detects incoming objects",
            "range_km": 150.5,
            "active": true,
            "allocates": [{"@id": "pa:Antenna01"}, {"@id": "pa:Processor"}]
        });

        let text = extract_rich_semantic_content(&doc);

        // On vérifie que les données métiers sont là
        assert!(text.contains("Name: Radar Module"));
        assert!(text.contains("Description: Detects incoming objects"));
        assert!(text.contains("range_km: 150.5"));
        assert!(text.contains("active: true"));
        assert!(text.contains("allocates: [pa:Antenna01, pa:Processor]"));

        // On vérifie que les données techniques sont exclues
        assert!(!text.contains("@id"));
        assert!(!text.contains("_created_at"));
    }

    #[async_test]
    async fn test_store_lifecycle_with_sandbox() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let store = GraphStore::new(sandbox.domain_root.clone(), &manager)
            .await
            .unwrap();

        let doc =
            json_value!({ "name": "TelemetryModule", "description": "Gère les flux de données" });
        store
            .index_entity("la", "TM01", doc)
            .await
            .expect("L'indexation doit réussir.");

        store
            .link_entities(("la", "TM01"), "realizes", ("sa", "DataMonitoring"))
            .await
            .unwrap();

        let file_path = sandbox.domain_root.join("collections/la/TM01.json");
        let raw = fs::read_to_string_async(&file_path).await.unwrap();
        let final_doc: JsonValue = json::deserialize_from_str(&raw).unwrap();

        assert_eq!(final_doc["@id"], "la:TM01");
        assert_eq!(final_doc["realizes"][0]["@id"], "sa:DataMonitoring");
    }
}
