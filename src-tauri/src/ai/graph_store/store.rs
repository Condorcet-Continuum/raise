// FICHIER : src-tauri/src/ai/graph_store/store.rs

use crate::ai::memory::candle_store::CandleLocalStore;
use crate::ai::memory::{MemoryRecord, VectorStore};
use crate::ai::nlp::embeddings::EmbeddingEngine;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::jsonld::JsonLdProcessor;
use crate::utils::prelude::*; // 🎯 Façade Unique

#[derive(Clone)]
pub struct GraphStore {
    pub storage_path: PathBuf,
    pub vector_store: Option<SharedRef<CandleLocalStore>>,
    pub embedder: Option<SharedRef<AsyncMutex<EmbeddingEngine>>>,
    pub embedding_dim: usize,
    pub processor: JsonLdProcessor,
}

impl GraphStore {
    /// Initialise le GraphStore de manière asynchrone en respectant les points de montage.
    pub async fn new(storage_path: PathBuf, manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let app_config = AppConfig::get();
        let use_vectors =
            app_config.core.graph_mode == "internal" || app_config.core.graph_mode == "db";
        let embedding_dim = app_config.world_model.embedding_dim;

        let mut vector_store = None;
        let mut embedder = None;

        if use_vectors {
            user_info!(
                "MSG_GRAPH_STORE_VECTORS_START",
                json_value!({ "action": "initialize_native_vectorization" })
            );

            // 🎯 Match strict sur l'initialisation du moteur sémantique
            match EmbeddingEngine::new(manager).await {
                Ok(engine) => {
                    let device = AppConfig::device();
                    let v_dir = storage_path.join("vectors");
                    let v_store = CandleLocalStore::new(&v_dir, device);

                    // Chargement résilient : si le store est absent, on continue à vide
                    if let Err(e) = v_store.load().await {
                        user_trace!(
                            "INF_GRAPH_STORE_NEW",
                            json_value!({"path": v_dir, "status": "initialized_empty", "reason": e.to_string()})
                        );
                    }

                    embedder = Some(SharedRef::new(AsyncMutex::new(engine)));
                    vector_store = Some(SharedRef::new(v_store));
                }
                Err(e) => {
                    user_warn!(
                        "WRN_GRAPH_STORE_INIT_FAILED",
                        json_value!({ "error": e.to_string(), "hint": "Le store fonctionnera en mode purement documentaire." })
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

    /// Indexe une entité Arcadia avec normalisation JSON-LD et vectorisation sémantique.
    pub async fn index_entity(
        &self,
        manager: &CollectionsManager<'_>,
        collection: &str,
        id: &str,
        mut data: JsonValue,
    ) -> RaiseResult<()> {
        // 1. Normalisation forcée des identifiants (Zéro Dette)
        if self.processor.get_id(&data).is_none() {
            data["@id"] = json_value!(format!("{}:{}", collection, id));
        }
        if data.get("_id").is_none() {
            data["_id"] = json_value!(id.to_string());
        }

        // 2. Branche Vectorielle (Inférence + Persistance)
        let text_to_embed = extract_rich_semantic_content(&data);

        if let (Some(emb_mutex), Some(v_store)) = (&self.embedder, &self.vector_store) {
            if !text_to_embed.is_empty() {
                let mut engine = emb_mutex.lock().await;
                // 🎯 Match strict sur l'inférence
                if let Ok(vector) = engine.embed_query(&text_to_embed) {
                    data["embedding"] = json_value!(vector.clone());

                    let record = MemoryRecord {
                        id: id.to_string(),
                        content: text_to_embed,
                        metadata: json_value!({ "collection": collection }),
                        vectors: Some(vector),
                    };

                    // Persistance vectorielle ignorée si échec (non-bloquant pour la branche doc)
                    let _ = v_store
                        .add_documents(manager, collection, vec![record])
                        .await;
                    let _ = v_store.save().await;
                }
            }
        }

        // 3. Branche Documentaire (Source Of Truth) via le manager
        manager.upsert_document(collection, data).await?;

        Ok(())
    }

    /// Établit un lien sémantique typé entre deux entités MBSE.
    pub async fn link_entities(
        &self,
        manager: &CollectionsManager<'_>,
        from: (&str, &str),
        relation: &str,
        to: (&str, &str),
    ) -> RaiseResult<()> {
        let (from_col, from_id) = from;

        // 1. Récupération via Match strict
        let mut doc = match manager.get_document(from_col, from_id).await? {
            Some(d) => d,
            None => raise_error!(
                "ERR_GRAPH_LINK_SOURCE_MISSING",
                error = format!(
                    "Impossible de lier : la source {}:{} n'existe pas.",
                    from_col, from_id
                )
            ),
        };

        // 2. Patch JSON-LD sémantique
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

        // 3. Persistance du lien
        manager.update_document(from_col, from_id, doc).await?;

        Ok(())
    }
}

/// Construit une représentation textuelle riche du composant pour les embeddings.
pub fn extract_rich_semantic_content(data: &JsonValue) -> String {
    let mut parts = Vec::new();

    if let Some(obj) = data.as_object() {
        if let Some(name) = obj.get("name").and_then(|v| v.as_str()) {
            parts.push(format!("Name: {}", name));
        }
        if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
            parts.push(format!("Description: {}", desc));
        }

        for (key, value) in obj {
            // Exclusion des métadonnées techniques
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
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[test]
    fn test_rich_semantic_extraction() {
        let doc = json_value!({
            "@id": "la:Radar",
            "name": "Radar Module",
            "description": "Detection system",
            "active": true,
            "allocates": [{"@id": "pa:Antenna"}]
        });

        let text = extract_rich_semantic_content(&doc);
        assert!(text.contains("Name: Radar Module"));
        assert!(text.contains("active: true"));
        assert!(text.contains("allocates: [pa:Antenna]"));
        assert!(!text.contains("@id"));
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_lifecycle_with_sandbox() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 FIX MOUNT POINTS : Utilisation du domaine système configuré
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        AgentDbSandbox::mock_db(&manager).await?;

        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );

        manager.create_collection("la", &schema_uri).await?;
        manager.create_collection("sa", &schema_uri).await?;

        let store = GraphStore::new(sandbox.domain_root.clone(), &manager).await?;
        let doc = json_value!({ "name": "Telemetry", "description": "Data stream" });

        // Test Indexation
        store.index_entity(&manager, "la", "T1", doc).await?;

        // Test Liaison
        store
            .link_entities(&manager, ("la", "T1"), "realizes", ("sa", "Monitoring"))
            .await?;

        let final_doc = match manager.get_document("la", "T1").await? {
            Some(d) => d,
            None => panic!("Document non trouvé après indexation"),
        };

        assert_eq!(final_doc["@id"], "la:T1");
        assert_eq!(final_doc["realizes"][0]["@id"], "sa:Monitoring");

        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_resilience_missing_source() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        AgentDbSandbox::mock_db(&manager).await?;

        let store = GraphStore::new(sandbox.domain_root.clone(), &manager).await?;

        // Tentative de lier une entité qui n'existe pas
        let result = store
            .link_entities(&manager, ("void", "99"), "rel", ("sa", "S1"))
            .await;

        match result {
            Err(AppError::Structured(err)) => assert_eq!(err.code, "ERR_GRAPH_LINK_SOURCE_MISSING"),
            _ => panic!("Le moteur aurait dû lever ERR_GRAPH_LINK_SOURCE_MISSING"),
        }
        Ok(())
    }

    #[async_test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_store_initialization_no_vectors() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // On initialise sans mock NLP : le GraphStore doit passer en mode dégradé documentaire sans crasher
        let store = GraphStore::new(sandbox.domain_root.clone(), &manager).await?;

        assert!(
            store.vector_store.is_none() || !sandbox.config.core.graph_mode.contains("internal")
        );
        Ok(())
    }
}
