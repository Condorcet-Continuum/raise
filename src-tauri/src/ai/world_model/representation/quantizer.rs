// FICHIER : src-tauri/src/ai/world_model/representation/quantizer.rs

use crate::utils::prelude::*;
use candle_core::{Module, Tensor};
use candle_nn::{Embedding, VarBuilder};

/// Module de Quantification Vectorielle (VQ-VAE style).
/// Il mappe un vecteur continu vers l'index du vecteur le plus proche dans le codebook.
pub struct VectorQuantizer {
    /// Le dictionnaire des concepts (Codebook)
    /// Shape: [num_embeddings, embedding_dim]
    embedding: Embedding,
}

impl VectorQuantizer {
    /// Initialise un nouveau Quantizer
    /// * `num_embeddings`: Taille du vocabulaire (K)
    /// * `embedding_dim`: Dimension des vecteurs (D)
    pub fn new(num_embeddings: usize, embedding_dim: usize, vb: VarBuilder) -> RaiseResult<Self> {
        // On initialise l'embedding table via Candle
        let embedding = candle_nn::embedding(num_embeddings, embedding_dim, vb)
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(Self { embedding })
    }

    /// Fonction principale : Transforme un vecteur d'entrée en Token (Index)
    /// Input: [Batch, Dim]
    /// Output: [Batch] (Indices des concepts les plus proches)
    pub fn tokenize(&self, z: &Tensor) -> RaiseResult<Tensor> {
        // 1. Calcul de la distance euclidienne au carré avec tous les vecteurs du codebook
        // ||z - e||^2 = ||z||^2 + ||e||^2 - 2 <z, e>

        // a. Carré de l'entrée : ||z||^2
        // ✅ Conversion des erreurs Candle pour sqr et sum
        let z_sq = z
            .sqr()
            .map_err(|e| AppError::from(e.to_string()))?
            .sum_keepdim(1)
            .map_err(|e| AppError::from(e.to_string()))?;

        // b. Carré du codebook : ||e||^2
        let w = self.embedding.embeddings();
        let w_sq = w
            .sqr()
            .map_err(|e| AppError::from(e.to_string()))?
            .sum_keepdim(1)
            .map_err(|e| AppError::from(e.to_string()))?
            .t()
            .map_err(|e| AppError::from(e.to_string()))?;

        // c. Produit scalaire : <z, e>
        let w_t = w.t().map_err(|e| AppError::from(e.to_string()))?;
        let zw = z.matmul(&w_t).map_err(|e| AppError::from(e.to_string()))?;

        // d. Assemblage de la distance
        // distance[i, j] = z_sq[i] + w_sq[j] - 2 * zw[i, j]
        let zw2 = (zw * 2.0).map_err(|e| AppError::from(e.to_string()))?;
        let dist = z_sq
            .broadcast_add(&w_sq)
            .map_err(|e| AppError::from(e.to_string()))?
            .broadcast_sub(&zw2)
            .map_err(|e| AppError::from(e.to_string()))?;

        // 2. Recherche du plus proche voisin (Argmin)
        let indices = dist.argmin(1).map_err(|e| AppError::from(e.to_string()))?;

        Ok(indices)
    }

    /// Décode un Token pour retrouver son vecteur prototype
    /// Input: [Batch] (Indices)
    /// Output: [Batch, Dim]
    pub fn decode(&self, indices: &Tensor) -> RaiseResult<Tensor> {
        let vectors = self
            .embedding
            .forward(indices)
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(vectors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    #[test]
    fn test_quantizer_logic() {
        // 1. Setup : Un petit Codebook de 2 vecteurs en 2D
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        let vq = VectorQuantizer::new(2, 2, vb.pp("vq")).unwrap();

        // --- Test de Dimensions ---
        let input = Tensor::randn(0f32, 1f32, (1, 2), &Device::Cpu).unwrap();
        let token = vq.tokenize(&input).unwrap();

        assert_eq!(token.dims(), &[1]);

        let decoded = vq.decode(&token).unwrap();
        assert_eq!(decoded.dims(), &[1, 2]);
    }

    #[test]
    fn test_nearest_neighbor_math() {
        let dev = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &dev);

        let vq = VectorQuantizer::new(2, 2, vb.pp("vq")).unwrap();

        // On récupère le vecteur #0 du codebook
        let codebook = vq.embedding.embeddings();
        let target_vec = codebook.get(0).unwrap().unsqueeze(0).unwrap();

        // On crée une entrée très proche
        let noise = Tensor::new(&[[0.001f32, 0.001]], &dev).unwrap();
        let input = (target_vec + noise).unwrap();

        // Tokenize
        let index = vq.tokenize(&input).unwrap();

        // CORRECTION : On extrait la valeur d'un tenseur de dimension 1 via to_vec1
        let indices_vec = index.to_vec1::<u32>().unwrap();
        let idx_scalar = indices_vec[0];

        assert_eq!(idx_scalar, 0);
    }
}
