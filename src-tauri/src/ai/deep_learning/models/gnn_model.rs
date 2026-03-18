// FICHIER : src-tauri/src/ai/deep_learning/models/gnn_model.rs
use crate::utils::prelude::*;
use candle_core::Tensor;
use candle_nn::VarBuilder;

use crate::ai::deep_learning::layers::gnn_layer::GcnLayer;
use crate::ai::graph_store::adjacency::GraphAdjacency;

/// Le modèle GNN complet spécialisé pour l'ontologie Arcadia.
/// 🎯 Mode "Production" : Exploite le Sparse Message Passing pour économiser la VRAM.
pub struct ArcadiaGnnModel {
    pub layer1: GcnLayer,
    pub layer2: GcnLayer,
}

impl ArcadiaGnnModel {
    /// Initialise un modèle GNN à 2 couches de manière asynchrone.
    pub async fn new(
        in_dim: usize,
        hidden_dim: usize,
        out_dim: usize,
        vb: VarBuilder<'_>,
    ) -> RaiseResult<Self> {
        // Initialisation de la Couche 1 (Agrégation locale)
        let layer1 = GcnLayer::new(in_dim, hidden_dim, vb.pp("layer1")).await?;

        // Initialisation de la Couche 2 (Contexte global / Sémantique dense)
        let layer2 = GcnLayer::new(hidden_dim, out_dim, vb.pp("layer2")).await?;

        user_info!("🧠 [GNN] Modèle Arcadia initialisé (Mode Sparse).");

        Ok(Self { layer1, layer2 })
    }

    /// Exécute la propagation complète sur le graphe via le Sparse Message Passing.
    /// Utilise les indices des arêtes (edges) plutôt qu'une matrice dense [N, N].
    pub async fn forward(
        &self,
        edge_src: &Tensor,
        edge_dst: &Tensor,
        features: &Tensor,
    ) -> RaiseResult<Tensor> {
        // Passe 1 : Agrégation des voisins directs via les arêtes
        let hidden = self.layer1.forward(edge_src, edge_dst, features).await?;

        // Passe 2 : Agrégation des voisins de niveau 2
        let output = self.layer2.forward(edge_src, edge_dst, &hidden).await?;

        Ok(output)
    }

    /// Calcule la similarité cosinus entre deux composants Arcadia après transformation GNN.
    pub async fn compute_similarity(
        &self,
        embeddings: &Tensor,
        adj_data: &GraphAdjacency,
        uri_a: &str,
        uri_b: &str,
    ) -> RaiseResult<f32> {
        // 1. Récupération stricte des index
        let idx_a = adj_data.uri_to_index.get(uri_a).cloned().ok_or_else(|| {
            build_error!(
                "ERR_GNN_URI_NOT_FOUND",
                error = format!("URI {} introuvable", uri_a)
            )
        })?;

        let idx_b = adj_data.uri_to_index.get(uri_b).cloned().ok_or_else(|| {
            build_error!(
                "ERR_GNN_URI_NOT_FOUND",
                error = format!("URI {} introuvable", uri_b)
            )
        })?;

        let data = embeddings
            .to_vec2::<f32>()
            .map_err(|e| build_error!("ERR_GNN_TENSOR_EXPORT", error = e.to_string()))?;

        let vec_a = &data[idx_a];
        let vec_b = &data[idx_b];

        // 2. Calcul de la similarité cosinus (A·B) / (||A||*||B||)
        let mut dot = 0.0;
        let mut norm_a = 0.0;
        let mut norm_b = 0.0;

        for (a, b) in vec_a.iter().zip(vec_b.iter()) {
            dot += a * b;
            norm_a += a * a;
            norm_b += b * b;
        }

        // 🎯 OPTIMISATION PROD : Epsilon de sécurité pour éviter les divisions par zéro (NaN)
        let epsilon = 1e-8_f32;

        Ok(dot / ((norm_a + epsilon).sqrt() * (norm_b + epsilon).sqrt()))
    }
}

// =========================================================================
// TESTS UNITAIRES (VALIDATION DU MODÈLE SPARSE)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    #[async_test]
    async fn test_gnn_model_sparse_flow() {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        // Modèle : In=4, Hidden=8, Out=2
        let model = ArcadiaGnnModel::new(4, 8, 2, vb).await.unwrap();

        // Nœuds et Edges factices (3 nœuds liés en chaîne + self-loops)
        let edge_src = Tensor::new(&[0u32, 1, 0, 1, 2], &device).unwrap();
        let edge_dst = Tensor::new(&[1u32, 2, 0, 1, 2], &device).unwrap();
        let feat = Tensor::zeros((3, 4), DType::F32, &device).unwrap();

        let output = model.forward(&edge_src, &edge_dst, &feat).await;

        assert!(output.is_ok(), "Le Forward complet du modèle a échoué.");
        assert_eq!(output.unwrap().dims(), &[3, 2]);
    }

    #[async_test]
    async fn test_similarity_logic_with_epsilon() {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);
        let model = ArcadiaGnnModel::new(2, 2, 2, vb).await.unwrap();

        let mut uri_map = UnorderedMap::new();
        uri_map.insert("la:Function".to_string(), 0);
        uri_map.insert("sa:Component".to_string(), 1);

        let adj_mock = GraphAdjacency {
            uri_to_index: uri_map,
            index_to_uri: vec!["la:Function".to_string(), "sa:Component".to_string()],
            matrix: Tensor::eye(2, DType::F32, &device).unwrap(),
        };

        // Deux vecteurs "Zéro" -> Testera l'Epsilon
        let embeddings_zero = Tensor::zeros((2, 2), DType::F32, &device).unwrap();
        let sim_zero = model
            .compute_similarity(&embeddings_zero, &adj_mock, "la:Function", "sa:Component")
            .await
            .unwrap();

        assert!(
            sim_zero.is_finite(),
            "L'Epsilon n'a pas protégé contre la division par zéro !"
        );

        // 🎯 FIX : On force le type en f32 avec le suffixe `_f32`
        let embeddings =
            Tensor::from_vec(vec![1.0_f32, 0.0_f32, 1.0_f32, 0.0_f32], (2, 2), &device).unwrap();
        let sim = model
            .compute_similarity(&embeddings, &adj_mock, "la:Function", "sa:Component")
            .await
            .unwrap();

        assert!(
            (sim - 1.0).abs() < 1e-5,
            "Les vecteurs identiques doivent avoir une similarité de 1.0"
        );
    }

    #[async_test]
    async fn test_gnn_message_passing_convergence_mbse() {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        // Initialisation déterministe des poids pour garantir un test stable
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        // Modèle : In=4, Hidden=8, Out=4
        let model = ArcadiaGnnModel::new(4, 8, 4, vb).await.unwrap();

        // 1. MOCK DE LA TOPOLOGIE MBSE (4 Nœuds)
        let mut uri_map = UnorderedMap::new();
        uri_map.insert("la:F1".to_string(), 0);
        uri_map.insert("la:F2".to_string(), 1);
        uri_map.insert("sa:S1".to_string(), 2);
        uri_map.insert("pa:P1".to_string(), 3);

        let adj_mock = GraphAdjacency {
            uri_to_index: uri_map,
            index_to_uri: vec![
                "la:F1".to_string(),
                "la:F2".to_string(),
                "sa:S1".to_string(),
                "pa:P1".to_string(),
            ],
            matrix: Tensor::eye(4, DType::F32, &device).unwrap(),
        };

        // 2. LISTE DES ARÊTES (Sparse)
        // Self-loops (chaque nœud se regarde lui-même)
        let mut src = vec![0u32, 1, 2, 3];
        let mut dst = vec![0u32, 1, 2, 3];

        // Liens bidirectionnels de réalisation (F1 <-> S1) et (F1 <-> F2)
        src.extend_from_slice(&[0, 2, 0, 1]);
        dst.extend_from_slice(&[2, 0, 1, 0]);

        let edge_src = Tensor::new(src.as_slice(), &device).unwrap();
        let edge_dst = Tensor::new(dst.as_slice(), &device).unwrap();

        // 3. VECTEURS SÉMANTIQUES INITIAUX (Orthogonaux - Matrice Identité)
        // F1 = [1,0,0,0], F2 = [0,1,0,0], S1 = [0,0,1,0], P1 = [0,0,0,1]
        let features = Tensor::eye(4, DType::F32, &device).unwrap();

        // 4. MESURE INITIALE (0.0 car orthogonaux)
        let sim_init = model
            .compute_similarity(&features, &adj_mock, "la:F1", "sa:S1")
            .await
            .unwrap();
        assert!(
            (sim_init - 0.0).abs() < 1e-5,
            "La similarité initiale doit être 0.0"
        );

        // 5. PROPAGATION GNN (L'IA réfléchit)
        let final_embeddings = model
            .forward(&edge_src, &edge_dst, &features)
            .await
            .unwrap();

        // 6. MESURES FINALES ET PREUVES
        let sim_final_connected = model
            .compute_similarity(&final_embeddings, &adj_mock, "la:F1", "sa:S1")
            .await
            .unwrap();

        let sim_final_isolated = model
            .compute_similarity(&final_embeddings, &adj_mock, "la:F1", "pa:P1")
            .await
            .unwrap();

        // A. Le GNN doit avoir rapproché F1 et S1
        assert!(
            sim_final_connected > 0.1,
            "Le Message Passing n'a pas réussi à rapprocher sémantiquement F1 et S1 ! (Sim: {})",
            sim_final_connected
        );

        // B. Les nœuds connectés doivent être significativement plus proches que le nœud isolé
        assert!(
            sim_final_connected > sim_final_isolated,
            "Le GNN ne différencie pas la topologie ! Connecté: {}, Isolé: {}",
            sim_final_connected,
            sim_final_isolated
        );
    }
}
