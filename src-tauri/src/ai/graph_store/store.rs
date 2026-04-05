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
    /// 🎯 OPTIMISATION : Utilisation exclusive du CollectionsManager pour l'écriture.
    pub async fn index_entity(
        &self,
        manager: &CollectionsManager<'_>,
        collection: &str,
        id: &str,
        mut data: JsonValue,
    ) -> RaiseResult<()> {
        // 1. Normalisation JSON-LD et DB
        if self.processor.get_id(&data).is_none() {
            data["@id"] = json_value!(format!("{}:{}", collection, id));
        }
        if data.get("_id").is_none() {
            data["_id"] = json_value!(id.to_string());
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

                    // 🎯 FIX ICI : On passe le paramètre `manager` !
                    let _ = v_store
                        .add_documents(manager, collection, vec![record])
                        .await;
                    let _ = v_store.save().await;
                }
            }
        }

        // 3. Branche Documentaire (Persistence via JSON-DB pour bénéficier de la validation et des index)
        manager.upsert_document(collection, data).await?;

        Ok(())
    }

    /// Établit un lien sémantique typé entre deux composants Arcadia.
    /// 🎯 OPTIMISATION : Utilisation exclusive du CollectionsManager.
    pub async fn link_entities(
        &self,
        manager: &CollectionsManager<'_>,
        from: (&str, &str),
        relation: &str,
        to: (&str, &str),
    ) -> RaiseResult<()> {
        let (from_col, from_id) = from;

        // 1. Lecture propre via la BDD
        let mut doc = match manager.get_document(from_col, from_id).await? {
            Some(d) => d,
            None => raise_error!(
                "ERR_GNN_LINK_FROM_NOT_FOUND",
                error = format!("Entité {}:{} introuvable", from_col, from_id)
            ),
        };

        // 2. Modification du JSON en mémoire
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

        // 3. Écriture propre via la BDD (Patch sémantique)
        manager.update_document(from_col, from_id, doc).await?;

        Ok(())
    }
}

/// 🎯 OPTIMISATION PROD : Construit une représentation textuelle riche du composant
/// pour maximiser la pertinence des embeddings vectoriels.
pub fn extract_rich_semantic_content(data: &JsonValue) -> String {
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

        AgentDbSandbox::mock_db(&manager).await.unwrap();
        manager
            .create_collection(
                "la",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();
        manager
            .create_collection(
                "sa",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await
            .unwrap();

        let store = GraphStore::new(sandbox.domain_root.clone(), &manager)
            .await
            .unwrap();

        let doc =
            json_value!({ "name": "TelemetryModule", "description": "Gère les flux de données" });

        // 🎯 On passe le manager
        store
            .index_entity(&manager, "la", "TM01", doc)
            .await
            .expect("L'indexation doit réussir.");

        // 🎯 On passe le manager pour lier
        store
            .link_entities(
                &manager,
                ("la", "TM01"),
                "realizes",
                ("sa", "DataMonitoring"),
            )
            .await
            .unwrap();

        // On lit de manière sémantique au lieu du FS brut !
        let final_doc = manager
            .get_document("la", "TM01")
            .await
            .unwrap()
            .expect("Le document devrait exister");

        assert_eq!(final_doc["@id"], "la:TM01");
        assert_eq!(final_doc["realizes"][0]["@id"], "sa:DataMonitoring");
    }
}
