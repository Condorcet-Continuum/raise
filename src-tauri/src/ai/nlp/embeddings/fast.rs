use crate::utils::prelude::*;

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub struct FastEmbedEngine {
    model: TextEmbedding,
}

impl FastEmbedEngine {
    pub fn new() -> RaiseResult<Self> {
        // CORRECTION : Utilisation de .with_show_download_progress(true)
        let options =
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true);

        let model = match TextEmbedding::try_new(options) {
            Ok(m) => m,
            Err(e) => raise_error!(
                "ERR_AI_FASTEMBED_INIT",
                error = e,
                context = json!({
                    "provider": "FastEmbed",
                    "action": "initialize_text_embedding"
                })
            ),
        };
        Ok(Self { model })
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> RaiseResult<Vec<Vec<f32>>> {
        // On sauvegarde la taille pour le contexte d'erreur avant que 'texts' ne soit consommé
        let batch_size = texts.len();

        // Capture explicite du résultat du modèle
        match self.model.embed(texts, None) {
            Ok(embeddings) => Ok(embeddings),
            Err(e) => {
                // Structuration standardisée de l'erreur
                raise_error!(
                    "ERR_AI_EMBEDDINGS_BATCH",
                    error = e.to_string(),
                    context = serde_json::json!({
                        "action": "embed_batch",
                        "batch_size": batch_size,
                        "hint": "Le modèle d'embedding a échoué à traiter ce lot."
                    })
                );
            }
        }
    }

    pub fn embed_query(&mut self, text: &str) -> RaiseResult<Vec<f32>> {
        // 1. Appel au modèle avec interception d'erreur de bibliothèque
        let embeddings = match self.model.embed(vec![text.to_string()], None) {
            Ok(e) => e,
            Err(e) => raise_error!(
                "ERR_AI_EMBEDDING_GEN_FAIL",
                error = e,
                context = json!({ "text_len": text.len() })
            ),
        };

        // 2. Extraction sécurisée du premier vecteur via let-else
        let Some(vector) = embeddings.into_iter().next() else {
            raise_error!(
                "ERR_AI_EMBEDDING_EMPTY",
                error = "Le modèle n'a produit aucun vecteur pour cette requête",
                context = json!({ "text_len": text.len(), "action": "embed_query" })
            );
        };

        Ok(vector)
    }
}

// --- TESTS UNITAIRES ---
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fast_embed_single() {
        let mut engine = FastEmbedEngine::new().expect("Init failed");
        let vec = engine
            .embed_query("Ceci est un test")
            .expect("Embedding failed");

        // BGE-Small-EN-V1.5 fait 384 dimensions
        assert_eq!(vec.len(), 384);

        // Vérification basique que le vecteur n'est pas vide/zéro
        assert!(vec.iter().any(|&x| x != 0.0));
    }

    #[test]
    fn test_fast_embed_batch() {
        let mut engine = FastEmbedEngine::new().expect("Init failed");
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
