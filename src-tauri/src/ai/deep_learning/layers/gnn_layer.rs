// FICHIER : src-tauri/src/ai/deep_learning/layers/gnn_layer.rs

use crate::utils::prelude::*;
use candle_core::{Module, Tensor};
use candle_nn::{linear, Linear, VarBuilder};

/// Une couche de convolution sur graphe (GCN) optimisée pour Arcadia.
/// 🎯 Utilise le "Sparse Message Passing" pour éviter l'explosion mémoire O(N^2).
pub struct GcnLayer {
    /// Transformation linéaire (Poids W et Biais b)
    pub transform: Linear,
    pub in_dim: usize,
    pub out_dim: usize,
}

impl GcnLayer {
    /// Initialise une nouvelle couche de manière asynchrone.
    pub async fn new(in_dim: usize, out_dim: usize, vb: VarBuilder<'_>) -> RaiseResult<Self> {
        let transform = match linear(in_dim, out_dim, vb) {
            Ok(l) => l,
            Err(e) => {
                raise_error!(
                    "ERR_GNN_LAYER_INIT",
                    error = e,
                    context = json_value!({ "in_dim": in_dim, "out_dim": out_dim })
                );
            }
        };

        Ok(Self {
            transform,
            in_dim,
            out_dim,
        })
    }

    /// Exécute la passe avant (Forward Pass) en mode Creux (Sparse).
    /// H_new = ReLU( Aggregation(H_voisins) * W + b )
    pub async fn forward(
        &self,
        edge_src: &Tensor, // 🎯 [E] Indices des nœuds sources
        edge_dst: &Tensor, // 🎯 [E] Indices des nœuds cibles
        features: &Tensor, // 🎯 [N, D] Matrice des caractéristiques
    ) -> RaiseResult<Tensor> {
        let feat_dims = features.dims();

        // 1. Validation de l'intégrité
        if feat_dims.len() != 2 {
            raise_error!(
                "ERR_GNN_INVALID_SHAPE",
                error = "Les features doivent être une matrice 2D [N, D].",
                context = json_value!({ "feat": feat_dims })
            );
        }

        // 2. SPARSE MESSAGE PASSING : O(E * D) au lieu de O(N^2 * D)
        // a. On copie les caractéristiques des nœuds sources
        let h_src = match features.index_select(edge_src, 0) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_INDEX_SELECT", error = e),
        };

        // b. On prépare un tenseur vierge [N, D] pour accumuler les messages
        let mut h_agg = match features.zeros_like() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_ZEROS_LIKE", error = e),
        };

        // c. On propage et additionne les messages vers les nœuds cibles
        h_agg = match h_agg.index_add(edge_dst, &h_src, 0) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_INDEX_ADD", error = e),
        };

        // 3. Transformation Sémantique & Activation : ReLU(Aggregated * W + b)
        let transformed = match self.transform.forward(&h_agg) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_LINEAR_TRANSFORM", error = e),
        };

        let activated = match transformed.relu() {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_ACTIVATION_RELU", error = e),
        };

        Ok(activated)
    }
}

// =========================================================================
// TESTS UNITAIRES (VALIDATION MATHÉMATIQUE SPARSE)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    #[async_test]
    async fn test_gcn_layer_sparse_forward_math() {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        // Modèle : 2 entrées, 4 sorties
        let layer = GcnLayer::new(2, 4, vb).await.unwrap();

        // Features H : 3 nœuds, 2 dimensions
        let feat = Tensor::zeros((3, 2), DType::F32, &device).unwrap();

        // Edges (Arêtes) : Nœud 0 -> Nœud 1, Nœud 1 -> Nœud 2, et self-loops
        let edge_src = Tensor::new(&[0u32, 1, 0, 1, 2], &device).unwrap();
        let edge_dst = Tensor::new(&[1u32, 2, 0, 1, 2], &device).unwrap();

        let output = layer.forward(&edge_src, &edge_dst, &feat).await;

        assert!(output.is_ok(), "Le forward pass Sparse a échoué.");
        let out_tensor = output.unwrap();
        assert_eq!(
            out_tensor.dims(),
            &[3, 4],
            "La dimension de sortie doit être [N, out_dim]."
        );
    }
}
