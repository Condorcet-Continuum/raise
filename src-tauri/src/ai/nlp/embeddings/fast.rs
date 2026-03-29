// FICHIER : src-tauri/src/ai/nlp/embeddings/fast.rs

use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub struct FastEmbedEngine {
    model: TextEmbedding,
}

impl FastEmbedEngine {
    // 🎯 FIX : On prend le manager et on devient async pour lire la base de données
    pub async fn new(manager: &CollectionsManager<'_>) -> RaiseResult<Self> {
        let settings = AppConfig::get_component_settings(manager, "ai_nlp")
            .await
            .unwrap_or(json_value!({}));

        let model_name_str = settings
            .get("fastembed_model")
            .and_then(|v| v.as_str())
            .unwrap_or("BGESmallENV15");

        // Déduction dynamique du modèle (on pourrait ajouter d'autres variantes de FastEmbed)
        let embed_model = match model_name_str {
            "AllMiniLML6V2" => EmbeddingModel::AllMiniLML6V2,
            _ => EmbeddingModel::BGESmallENV15,
        };

        let options = InitOptions::new(embed_model).with_show_download_progress(true);

        let model = match TextEmbedding::try_new(options) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_AI_FASTEMBED_INIT",
                error = e,
                context = json_value!({
                    "provider": "FastEmbed",
                    "action": "initialize_text_embedding",
                    "model": model_name_str
                })
            ),
        };
        Ok(Self { model })
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        let batch_size = texts.len();

        match self.model.embed(texts, None) {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => {
                raise_error!(
                    "ERR_AI_EMBEDDINGS_BATCH",
                    error = e.to_string(),
                    context = json_value!({
                        "action": "embed_batch",
                        "batch_size": batch_size,
                        "provider": "FastEmbed",
                        "hint": "Le modèle d'embedding a échoué à traiter ce lot."
                    })
                );
            }
        }
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        let embeddings = match self.model.embed(vec![text.to_string()], None) {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_AI_EMBEDDING_GEN_FAIL",
                error = e,
                context = json_value!({ "text_len": text.len(), "provider": "FastEmbed" })
            ),
        };

        let Some(vector) = embeddings.into_iter().next() else {
            raise_error!(
                "ERR_AI_EMBEDDING_EMPTY",
                error = "Le modèle n'a produit aucun vecteur pour cette requête",
                context = json_value!({ "text_len": text.len(), "action": "embed_query" })
            );
        };

        Ok(vector)
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_fast_embed_single() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let mut engine = FastEmbedEngine::new(&manager).await.expect("Init failed");
        let vec = engine
            .embed_query("Ceci est un test")
            .expect("Embedding failed");

        // BGE-Small-EN-V1.5 fait 384 dimensions
        assert_eq!(vec.len(), 384);
        assert!(vec.iter().any(|&x| x != 0.0));
    }

    #[async_test]
    async fn test_fast_embed_batch() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        let mut engine = FastEmbedEngine::new(&manager).await.expect("Init failed");
        let inputs = vec![
            "Phrase 1".to_string(),
            "Phrase 2".to_string(),
            "Phrase 3".to_string(),
        ];

        let batch_res = engine.embed_batch(inputs).expect("Batch failed");
        assert_eq!(batch_res.len(), 3);
        assert_eq!(batch_res[0].len(), 384);
    }
}
