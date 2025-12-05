use anyhow::Result;
use async_trait::async_trait;

/// Contrat pour un moteur de vectorisation
#[async_trait]
pub trait Embedder: Send + Sync {
    /// Convertit un texte en vecteur de flottants
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;

    /// Convertit un lot de textes (batch)
    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>>;

    /// Dimension du vecteur (ex: 384, 768, 1536)
    fn dimension(&self) -> usize;
}

/// Implémentation Dummy (pour les tests sans modèle chargé)
pub struct DummyEmbedder;

#[async_trait]
impl Embedder for DummyEmbedder {
    async fn embed(&self, _text: &str) -> Result<Vec<f32>> {
        // Retourne un vecteur de zéros
        Ok(vec![0.0; 384])
    }

    async fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        Ok(vec![vec![0.0; 384]; texts.len()])
    }

    fn dimension(&self) -> usize {
        384
    }
}
