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
        // On remplace le map_err par un match pour extraire l'Embedding proprement
        let embedding = match candle_nn::embedding(num_embeddings, embedding_dim, vb) {
            Ok(emb) => emb,
            Err(e) => {
                raise_error!(
                    "ERR_AI_QUANTIZER_INIT_FAILED",
                    error = e,
                    context = json!({
                        "num_embeddings": num_embeddings,
                        "embedding_dim": embedding_dim,
                        "hint": "Échec de l'initialisation du dictionnaire d'embeddings. Vérifiez que la taille du vocabulaire et la dimension correspondent aux poids fournis."
                    })
                )
            }
        };

        Ok(Self { embedding })
    }

    /// Fonction principale : Transforme un vecteur d'entrée en Token (Index)
    /// Input: [Batch, Dim]
    /// Output: [Batch] (Indices des concepts les plus proches)
    pub fn tokenize(&self, z: &Tensor) -> RaiseResult<Tensor> {
        // 1. Norme de l'entrée ||z||^2
        let z_sq = match z.sqr().and_then(|s| s.sum_keepdim(1)) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_QUANT_Z_NORM_FAILED", error = e),
        };

        // 2. Norme du codebook ||e||^2
        let w = self.embedding.embeddings();
        let w_sq = match w.sqr().and_then(|s| s.sum_keepdim(1)).and_then(|s| s.t()) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_QUANT_W_NORM_FAILED", error = e),
        };

        // 3. Produit scalaire <z, e> et assemblage
        let w_t = match w.t() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_QUANT_W_TRANSPOSE_FAILED", error = e),
        };

        let zw = match z.matmul(&w_t) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_QUANT_MATMUL_FAILED", error = e),
        };

        let zw2 = match &zw * 2.0 {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_QUANT_SCALAR_MUL_FAILED", error = e),
        };

        let dist = match z_sq
            .broadcast_add(&w_sq)
            .and_then(|res| res.broadcast_sub(&zw2))
        {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_AI_QUANT_BROADCAST_FAILED",
                error = e,
                context = json!({
                    "z_shape": format!("{:?}", z.shape()),
                    "w_shape": format!("{:?}", w.shape())
                })
            ),
        };

        // 4. Recherche du plus proche voisin (Argmin)
        match dist.argmin(1) {
            Ok(indices) => Ok(indices),
            Err(e) => raise_error!("ERR_AI_QUANT_ARGMIN_FAILED", error = e),
        }
    }

    /// Décode un Token pour retrouver son vecteur prototype
    /// Input: [Batch] (Indices)
    /// Output: [Batch, Dim]
    pub fn decode(&self, indices: &Tensor) -> RaiseResult<Tensor> {
        // La méthode .forward() sur une couche d'Embedding fait le lookup des index
        match self.embedding.forward(indices) {
            Ok(vectors) => Ok(vectors),
            Err(e) => raise_error!(
                "ERR_AI_QUANT_DECODE_FAILED",
                error = e,
                context = json!({
                    "action": "codebook_lookup",
                    "indices_shape": format!("{:?}", indices.shape()),
                    "hint": "Échec de la récupération des vecteurs. Vérifiez que les indices ne dépassent pas la taille du vocabulaire (num_embeddings)."
                })
            ),
        }
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
