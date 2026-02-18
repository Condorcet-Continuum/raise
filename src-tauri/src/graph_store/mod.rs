pub mod surreal_impl;

use crate::utils::{io::PathBuf, prelude::*, Arc};

use self::surreal_impl::SurrealClient;
use crate::ai::nlp::embeddings::EmbeddingEngine; // Import du moteur NLP
use tokio::sync::Mutex; // Nécessaire car EmbeddingEngine a besoin de mutabilité

#[derive(Clone)]
pub struct GraphStore {
    backend: SurrealClient,
    // On garde le moteur optionnel et thread-safe
    embedder: Option<Arc<Mutex<EmbeddingEngine>>>,
}

impl GraphStore {
    pub async fn new(storage_path: PathBuf) -> Result<Self> {
        let backend = SurrealClient::init(storage_path).await?;
        let app_config = AppConfig::get();

        let use_vectors =
            app_config.core.graph_mode == "internal" || app_config.core.graph_mode == "db";

        let embedder = if use_vectors {
            println!(
                "🕸️ [GraphStore] Vectorisation activée (Hybrid Search) via mode: {}",
                app_config.core.graph_mode
            );
            match EmbeddingEngine::new() {
                Ok(engine) => Some(Arc::new(Mutex::new(engine))),
                Err(e) => {
                    eprintln!("⚠️ Echec init EmbeddingEngine pour GraphStore: {}", e);
                    None
                }
            }
        } else {
            None
        };

        Ok(Self { backend, embedder })
    }

    /// Indexe une entité. Si le vector store est actif, on calcule l'embedding.
    pub async fn index_entity(
        &self,
        collection: &str,
        id: &str,
        mut data: serde_json::Value, // mut pour pouvoir injecter le vecteur
    ) -> Result<()> {
        // 2. Vectorisation Automatique
        if let Some(embedder_mutex) = &self.embedder {
            // On essaie de trouver du texte pertinent dans l'objet JSON
            let text_to_embed = extract_text_content(&data);

            if !text_to_embed.is_empty() {
                let mut engine = embedder_mutex.lock().await;
                // Calcul du vecteur (384 dimensions)
                if let Ok(vector) = engine.embed_query(&text_to_embed) {
                    // Injection dans le champ "embedding" réservé par SurrealDB
                    data["embedding"] = json!(vector);
                }
            }
        }

        // 3. Sauvegarde dans SurrealDB
        let _ = self.backend.upsert_node(collection, id, data).await?;
        Ok(())
    }

    /// Recherche hybride : Trouve les nœuds sémantiquement proches
    pub async fn search_similar(
        &self,
        collection: &str,
        query: &str,
        limit: usize,
    ) -> Result<Vec<serde_json::Value>> {
        if let Some(embedder_mutex) = &self.embedder {
            let mut engine = embedder_mutex.lock().await;
            // 1. Vectorisation de la requête
            let query_vector = engine.embed_query(query)?;

            // 2. Appel au backend SurrealDB
            self.backend
                .search_similar(collection, query_vector, limit)
                .await
        } else {
            // Fallback si vecteurs désactivés : on renvoie vide ou on pourrait faire une recherche texte classique
            Ok(vec![])
        }
    }

    pub async fn remove_entity(&self, collection: &str, id: &str) -> Result<()> {
        self.backend.delete_node(collection, id).await
    }

    pub async fn link_entities(
        &self,
        from: (&str, &str),
        relation: &str,
        to: (&str, &str),
    ) -> Result<()> {
        self.backend.create_edge(from, relation, to).await
    }

    pub fn backend(&self) -> &SurrealClient {
        &self.backend
    }
}

/// Helper : Extrait une chaîne représentative d'un objet JSON pour la vectorisation
fn extract_text_content(data: &serde_json::Value) -> String {
    // Priorité 1 : Champ "description"
    if let Some(desc) = data.get("description").and_then(|v| v.as_str()) {
        return desc.to_string();
    }
    // Priorité 2 : Champ "content"
    if let Some(content) = data.get("content").and_then(|v| v.as_str()) {
        return content.to_string();
    }
    // Priorité 3 : Champ "name"
    if let Some(name) = data.get("name").and_then(|v| v.as_str()) {
        return name.to_string();
    }
    // Fallback : Dump JSON (moins précis sémantiquement mais couvre tout)
    data.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::io::tempdir;

    #[tokio::test]
    async fn test_graph_vector_flag() {
        // 1. On initialise la configuration (let _ = ignore l'erreur si un autre test l'a déjà fait)
        let _ = AppConfig::init();

        let dir = tempdir().unwrap();
        let store = GraphStore::new(dir.path().to_path_buf()).await.unwrap();

        // 2. Indexer une entité avec du texte
        let data = json!({
            "name": "Moteur Électrique",
            "description": "Système de propulsion utilisant l'énergie électrique."
        });

        // L'insertion se fait. L'embedding sera calculé UNIQUEMENT SI la config l'autorise.
        store
            .index_entity("component", "engine", data)
            .await
            .unwrap();

        // 3. Récupération du nœud en base
        let node = store.backend.select("component", "engine").await.unwrap();
        let node_data = node.unwrap();

        // 4. VÉRIFICATION CONDITIONNELLE (Le cœur de la solution)
        let config = AppConfig::get();
        let is_vector_active =
            config.core.graph_mode == "internal" || config.core.graph_mode == "db";

        if is_vector_active {
            // CAS A : Les vecteurs SONT activés par la config
            assert!(
                node_data.get("embedding").is_some(),
                "Le champ 'embedding' DOIT être présent quand les vecteurs sont activés"
            );

            let vec_arr = node_data["embedding"].as_array().unwrap();
            assert_eq!(vec_arr.len(), 384, "La dimension du vecteur doit être 384");

            // Test de la recherche sémantique
            let results = store
                .search_similar("component", "propulsion", 1)
                .await
                .unwrap();
            assert!(
                !results.is_empty(),
                "La recherche doit trouver le composant"
            );
            println!("Score de similarité : {}", results[0]["score"]);
        } else {
            // CAS B : Les vecteurs NE SONT PAS activés par la config
            assert!(
                node_data.get("embedding").is_none(),
                "Le champ 'embedding' NE DOIT PAS exister puisque le mode graphique est '{}'",
                config.core.graph_mode
            );
            println!(
                "Test validé (Mode sans vecteur) : L'entité a bien été sauvegardée sans embedding."
            );
        }
    }
}
