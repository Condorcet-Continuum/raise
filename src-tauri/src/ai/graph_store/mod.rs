// FICHIER : src-tauri/src/ai/graph_store/mod.rs
use crate::utils::prelude::*;

use crate::ai::memory::{candle_store::CandleLocalStore, MemoryRecord, VectorStore};
use crate::ai::nlp::embeddings::EmbeddingEngine;
use crate::json_db::jsonld::JsonLdProcessor;

// 🎯 Imports mis à jour
use crate::json_db::collections::manager::CollectionsManager;

#[derive(Clone)]
pub struct GraphStore {
    storage_path: PathBuf,
    vector_store: Option<SharedRef<CandleLocalStore>>,
    embedder: Option<SharedRef<AsyncMutex<EmbeddingEngine>>>,
    embedding_dim: usize, // 🎯 On stocke la dimension SSOT
    processor: JsonLdProcessor,
}

impl GraphStore {
    // 🎯 AJOUT DU MANAGER DANS LA SIGNATURE
    pub async fn new(storage_path: PathBuf, manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let app_config = AppConfig::get();
        let use_vectors =
            app_config.core.graph_mode == "internal" || app_config.core.graph_mode == "db";

        // 🎯 On récupère la configuration du World Model
        let wm_config = &app_config.world_model;
        let embedding_dim = wm_config.embedding_dim;

        let mut vector_store = None;
        let mut embedder = None;

        if use_vectors {
            println!("🕸️ [GraphStore] Vectorisation Native activée (JSON-LD + Candle)");

            // 🎯 AJOUT DU MANAGER ET DU .await ICI
            match EmbeddingEngine::new(manager).await {
                Ok(engine) => {
                    embedder = Some(SharedRef::new(AsyncMutex::new(engine)));

                    // 🎯 ALIGNEMENT MATÉRIEL : Utilisation du GPU si activé dans la config
                    let device = AppConfig::device().clone();

                    let v_dir = storage_path.join("vectors");
                    let v_store = CandleLocalStore::new(&v_dir, &device);

                    // On précharge le store
                    let _ = v_store.load().await;
                    vector_store = Some(SharedRef::new(v_store));
                }
                Err(e) => eprintln!("⚠️ Echec init EmbeddingEngine pour GraphStore: {}", e),
            }
        }
        let processor = JsonLdProcessor::new(); // 🎯 Initialisation par défaut
        Ok(Self {
            storage_path,
            vector_store,
            embedder,
            embedding_dim,
            processor,
        })
    }

    /// Indexe une entité. Si le vector store est actif, on calcule l'embedding.
    pub async fn index_entity(
        &self,
        collection: &str,
        id: &str,
        mut data: JsonValue,
    ) -> RaiseResult<()> {
        // 🎯 OPTIMISATION JSON-LD 1 : Normalisation de l'ID
        if self.processor.get_id(&data).is_none() {
            data["@id"] = json_value!(format!("{}:{}", collection, id));
        }

        // 🎯 OPTIMISATION JSON-LD 2 : Validation stricte
        if let Err(e) = self
            .processor
            .validate_required_fields(&data, &["@id", "@type"])
        {
            eprintln!("⚠️ Warning Sémantique pour {}: {}", id, e);
        }

        let text_to_embed = extract_text_content(&data);

        // 1. Vectorisation Automatique (CandleLocalStore)
        if let (Some(emb_mutex), Some(v_store)) = (&self.embedder, &self.vector_store) {
            if !text_to_embed.is_empty() {
                let mut engine = emb_mutex.lock().await;
                if let Ok(vector) = engine.embed_query(&text_to_embed) {
                    data["embedding"] = json_value!(vector);

                    let record = MemoryRecord {
                        id: id.to_string(),
                        content: text_to_embed,
                        metadata: json_value!({ "collection": collection }),
                        vectors: Some(vector),
                    };

                    // 🎯 ALIGNEMENT SSOT : On utilise la dimension de la configuration
                    let _ = v_store
                        .init_collection(collection, self.embedding_dim as u64)
                        .await;
                    let _ = v_store.add_documents(collection, vec![record]).await;
                    let _ = v_store.save().await;
                }
            }
        }

        // 2. Sauvegarde dans le Système de Fichiers (JSON pur)
        let col_dir = self.storage_path.join("collections").join(collection);
        if !col_dir.exists() {
            if let Err(e) = fs::create_dir_all_async(&col_dir).await {
                raise_error!(
                    "ERR_GRAPH_STORAGE_INIT_FAILED",
                    error = e,
                    context = json_value!({
                        "action": "create_graph_directory",
                        "path": col_dir.to_string_lossy(),
                        "hint": "Impossible d'initialiser le dossier du Graph Store."
                    })
                )
            }
        }

        data["id"] = json_value!(id); // Force l'ID standardisé
        let file_path = col_dir.join(format!("{}.json", id));

        // 1. Sérialisation avec capture de contexte
        let json_str = match serde_json::to_string_pretty(&data) {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_DB_SERIALIZATION_FAILED",
                error = e,
                context = json_value!({
                    "action": "serialize_document_to_json",
                    "hint": "Les données contiennent probablement une structure incompatible avec JSON (références circulaires ou types non supportés)."
                })
            ),
        };

        // 2. Écriture atomique sur le disque
        if let Err(e) = fs::write_async(&file_path, json_str).await {
            raise_error!(
                "ERR_DB_WRITE_IO_FAILED",
                error = e,
                context = json_value!({
                    "action": "write_file_to_disk",
                    "path": file_path.to_string_lossy(),
                    "hint": "Vérifiez l'espace disque disponible et les permissions d'écriture sur le répertoire data_root."
                })
            );
        }

        Ok(())
    }

    /// Recherche hybride : Trouve les nœuds sémantiquement proches
    pub async fn search_similar(
        &self,
        collection: &str,
        query: &str,
        limit: usize,
    ) -> RaiseResult<Vec<JsonValue>> {
        if let (Some(emb_mutex), Some(v_store)) = (&self.embedder, &self.vector_store) {
            let mut engine = emb_mutex.lock().await;
            let query_vector = engine.embed_query(query)?;

            // Recherche des vecteurs via CandleLocalStore
            let records = v_store
                .search_similarity(collection, &query_vector, limit as u64, 0.4, None)
                .await?;

            let mut results = Vec::new();
            for rec in records {
                let file_path = self
                    .storage_path
                    .join("collections")
                    .join(collection)
                    .join(format!("{}.json", rec.id));
                if let Ok(content) = fs::read_to_string_async(&file_path).await {
                    if let Ok(doc) = json::deserialize_from_str::<JsonValue>(&content) {
                        results.push(doc);
                    }
                }
            }
            Ok(results)
        } else {
            Ok(vec![]) // Pas de recherche possible sans les vecteurs
        }
    }

    pub async fn remove_entity(&self, collection: &str, id: &str) -> RaiseResult<()> {
        let file_path = self
            .storage_path
            .join("collections")
            .join(collection)
            .join(format!("{}.json", id));
        let _ = fs::remove_file_async(&file_path).await;
        Ok(())
    }

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

        if let Ok(content) = fs::read_to_string_async(&file_path).await {
            if let Ok(mut doc) = json::deserialize_from_str::<JsonValue>(&content) {
                // Graphe orienté document : on ajoute la relation sous forme de lien URI (JSON-LD pattern)
                let target_uri = format!("{}:{}", to.0, to.1);

                if let Some(obj) = doc.as_object_mut() {
                    let rel_array = obj.entry(relation).or_insert(json_value!([]));
                    if let Some(arr) = rel_array.as_array_mut() {
                        let link = json_value!({ "@id": target_uri });
                        if !arr.contains(&link) {
                            arr.push(link);
                        }
                    }
                }

                // 1. Sérialisation sécurisée
                let json_str = match serde_json::to_string_pretty(&doc) {
                    Ok(s) => s,
                    Err(e) => {
                        raise_error!(
                            "ERR_DOC_SERIALIZATION_FAILED",
                            error = e,
                            context = json_value!({
                                "action": "serialize_document",
                                "hint": "Le document contient probablement des types incompatibles avec JSON ou des cycles."
                            })
                        )
                    }
                };

                // 2. Écriture atomique
                if let Err(e) = fs::write_async(&file_path, json_str).await {
                    raise_error!(
                        "ERR_DOC_WRITE_FAILED",
                        error = e,
                        context = json_value!({
                            "action": "persist_document_to_disk",
                            "path": file_path.to_string_lossy(),
                            "hint": "Vérifiez les permissions d'écriture et l'espace disque disponible."
                        })
                    )
                }
            }
        }
        Ok(())
    }
}

/// Helper : Extrait une chaîne représentative d'un objet JSON pour la vectorisation
fn extract_text_content(data: &JsonValue) -> String {
    if let Some(desc) = data.get("description").and_then(|v| v.as_str()) {
        return desc.to_string();
    }
    if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
        return content.to_string();
    }
    if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
        return name.to_string();
    }
    data.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{inject_mock_component, AgentDbSandbox};

    fn get_hf_lock() -> &'static AsyncMutex<()> {
        static LOCK: StaticCell<AsyncMutex<()>> = StaticCell::new();
        LOCK.get_or_init(|| AsyncMutex::new(()))
    }

    #[async_test]
    async fn test_native_graph_store_end_to_end() {
        let _guard = get_hf_lock().lock().await;
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );
        // 🎯 Injection du modèle NLP pour éviter le plantage
        inject_mock_component(
            &manager,
            "nlp",
            json_value!({ "model_name": "minilm", "rust_config_file": "config.json", "rust_tokenizer_file": "tokenizer.json", "rust_safetensors_file": "model.safetensors" })
        ).await;

        // 🎯 On passe le manager
        let store = GraphStore::new(sandbox.domain_root.clone(), &manager)
            .await
            .unwrap();

        // 1. Indexation
        let data = json_value!({
            "name": "Moteur Électrique",
            "description": "Système de propulsion 100% électrique."
        });
        store
            .index_entity("component", "engine1", data)
            .await
            .unwrap();

        // 2. Création d'un lien (Edge)
        let data_car = json_value!({ "name": "Voiture de sport" });
        store
            .index_entity("system", "car1", data_car)
            .await
            .unwrap();

        // car1 -> utilise -> engine1
        store
            .link_entities(("system", "car1"), "uses", ("component", "engine1"))
            .await
            .unwrap();

        // 3. Recherche Vectorielle (Si les vecteurs sont activés)
        if store.vector_store.is_some() {
            let results = store
                .search_similar("component", "propulsion", 1)
                .await
                .unwrap();
            assert!(
                !results.is_empty(),
                "La recherche vectorielle doit fonctionner nativement"
            );
            assert_eq!(results[0]["name"], "Moteur Électrique");
        }
    }
}
