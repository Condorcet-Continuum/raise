use anyhow::{Context, Result};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub struct FastEmbedEngine {
    model: TextEmbedding,
}

impl FastEmbedEngine {
    pub fn new() -> Result<Self> {
        // CORRECTION : Utilisation de .with_show_download_progress(true)
        let options =
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true);

        let model = TextEmbedding::try_new(options).context("❌ FastEmbed Init Failed")?;

        Ok(Self { model })
    }

    pub fn embed_batch(&mut self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        self.model.embed(texts, None)
    }

    pub fn embed_query(&mut self, text: &str) -> Result<Vec<f32>> {
        let embeddings = self.model.embed(vec![text.to_string()], None)?;
        embeddings
            .into_iter()
            .next()
            .context("No embedding generated")
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
