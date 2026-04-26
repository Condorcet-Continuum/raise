// FICHIER : src-tauri/src/ai/graph_store/engine.rs

use crate::ai::deep_learning::models::gnn_model::ArcadiaGnnModel;
use crate::ai::graph_store::adjacency::GraphAdjacency;
use crate::ai::graph_store::logic::ArcadiaLogic;
use crate::json_db::collections::manager::CollectionsManager;
use crate::utils::prelude::*;

pub struct GnnEngine {
    pub model: ArcadiaGnnModel,
    pub logic: ArcadiaLogic,
    pub varmap: NeuralWeightsMap,
    pub optimizer: NeuralOptimizerAdamW,
}

impl GnnEngine {
    /// 🏗️ INITIALISATION DU MOTEUR (MODE SPARSE)
    pub async fn new(
        manager: &CollectionsManager<'_>,
        in_dim: usize,
        hidden_dim: usize,
        device: &ComputeHardware,
    ) -> RaiseResult<Self> {
        let adj = GraphAdjacency::build_from_store(manager, device).await?;
        let logic = ArcadiaLogic::build_violation_matrix(&adj.index_to_uri, device)?;

        let varmap = NeuralWeightsMap::new();
        let vb = NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, device);

        let model = ArcadiaGnnModel::new(in_dim, hidden_dim, hidden_dim, vb).await?;

        // 🎯 FIX ERR_1 : Le champ correct est 'lr' et non 'learning_rate'
        let opt_config = OptimizerConfigAdamW {
            lr: 1e-3,
            ..Default::default()
        };

        // 🎯 FIX ERR_2 : L'optimiseur attend une liste de variables Vec<Var>, pas la VarMap elle-même
        let vars = varmap.all_vars();
        let optimizer = match NeuralOptimizerAdamW::new(vars, opt_config) {
            Ok(opt) => opt,
            Err(e) => raise_error!("ERR_GNN_ENGINE_OPT_INIT", error = e.to_string()),
        };

        Ok(Self {
            model,
            logic,
            varmap,
            optimizer,
        })
    }

    /// 🎓 ÉTAPE D'ENTRAÎNEMENT (BACKPROPAGATION)
    pub async fn train_step(
        &mut self,
        features: &NeuralTensor,
        adj: &GraphAdjacency,
        lambda_logic: f32,
    ) -> RaiseResult<f32> {
        // 1. Forward Pass : Calcul des embeddings [N, D]
        let embeddings = self
            .model
            .forward(&adj.edge_src, &adj.edge_dst, features)
            .await?;

        // 2. Calcul de la Perte Sémantique (Lien Positif)
        // 🎯 FIX DIMENSIONS : On extrait les vecteurs sources et cibles pour chaque arête [E, D]
        let h_src = match embeddings.index_select(&adj.edge_src, 0) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_LOSS_SELECT_SRC", error = e.to_string()),
        };
        let h_dst = match embeddings.index_select(&adj.edge_dst, 0) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_LOSS_SELECT_DST", error = e.to_string()),
        };

        // Produit scalaire par arête pour mesurer la force du lien prédit [E]
        let edge_scores = match h_src.mul(&h_dst).and_then(|t| t.sum_keepdim(1)) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_GNN_LOSS_DOT", error = e.to_string()),
        };

        // On veut que la similarité sur les liens existants tende vers 1.0 (MSE Loss simplifiée)
        let l_semantic = match edge_scores
            .ones_like()
            .and_then(|ones| edge_scores.sub(&ones))
        {
            Ok(diff) => match diff.sqr().and_then(|s| s.mean_all()) {
                Ok(l) => l,
                Err(e) => raise_error!("ERR_GNN_LOSS_MSE", error = e.to_string()),
            },
            Err(e) => raise_error!("ERR_GNN_LOSS_DIFF", error = e.to_string()),
        };

        // 3. Calcul de la Perte Logique (Neuro-Symbolique)
        // On génère la matrice de prédiction globale [N, N] pour la Logic Loss
        let predictions = match embeddings.matmul(&embeddings.transpose(0, 1).unwrap()) {
            Ok(p) => p,
            Err(e) => raise_error!("ERR_GNN_ENGINE_PRED_MATMUL", error = e.to_string()),
        };

        let l_logic = self.logic.compute_logic_loss(&predictions, lambda_logic)?;

        // 4. Somme et Backpropagation
        let total_loss = match l_semantic.add(&l_logic) {
            Ok(l) => l,
            Err(e) => raise_error!("ERR_GNN_ENGINE_LOSS_SUM", error = e.to_string()),
        };

        match self.optimizer.backward_step(&total_loss) {
            Ok(_) => (),
            Err(e) => raise_error!("ERR_GNN_ENGINE_BACKWARD", error = e.to_string()),
        }

        match total_loss.to_scalar::<f32>() {
            Ok(v) => Ok(v),
            Err(_) => Ok(0.0),
        }
    }

    /// 🔍 AUDIT DE L'ONTOLOGIE
    pub async fn audit_ontology(
        &self,
        features: &NeuralTensor,
        adj: &GraphAdjacency,
    ) -> RaiseResult<Vec<JsonValue>> {
        let mut reports = Vec::new();
        let embeddings = self
            .model
            .forward(&adj.edge_src, &adj.edge_dst, features)
            .await?;

        let src_vec = match adj.edge_src.to_vec1::<u32>() {
            Ok(v) => v,
            Err(e) => raise_error!("ERR_GNN_AUDIT_READ", error = e.to_string()),
        };

        let dst_vec = match adj.edge_dst.to_vec1::<u32>() {
            Ok(v) => v,
            Err(e) => raise_error!("ERR_GNN_AUDIT_READ", error = e.to_string()),
        };

        for (i, &u_idx) in src_vec.iter().enumerate() {
            let v_idx = dst_vec[i];
            let u_uri = &adj.index_to_uri[u_idx as usize];
            let v_uri = &adj.index_to_uri[v_idx as usize];

            // 🎯 FIX WARN_3 : On utilise 'sim' pour remplir le rapport
            let sim = self
                .model
                .compute_similarity(&embeddings, adj, u_uri, v_uri)
                .await?;

            if sim < 0.5 {
                reports.push(json_value!({
                    "type": "low_semantic_confidence",
                    "source": u_uri,
                    "target": v_uri,
                    "score": sim
                }));
            }
        }

        Ok(reports)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::{AgentDbSandbox, DbSandbox};

    #[async_test]
    #[serial_test::serial]
    #[cfg_attr(not(feature = "cuda"), ignore)]
    async fn test_gnn_engine_full_cycle_mbse() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let device = ComputeHardware::Cpu;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        // 🎯 FIX : Il faut créer la collection et insérer un document AVANT d'initialiser l'Engine
        let schema_uri = format!(
            "db://{}/{}/schemas/v1/db/generic.schema.json",
            config.mount_points.system.domain, config.mount_points.system.db
        );
        manager.create_collection("la", &schema_uri).await?;
        manager
            .insert_raw(
                "la",
                &json_value!({
                    "_id": "F1",
                    "@id": "la:F1",
                    "name": "Core Function"
                }),
            )
            .await?;

        // L'initialisation passera car le store n'est plus vide
        let mut engine = GnnEngine::new(&manager, 384, 128, &device).await?;

        let n_nodes = engine.logic.violation_matrix.dims()[0];
        let features = match NeuralTensor::zeros((n_nodes, 384), ComputeType::F32, &device) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TEST_ALLOC", error = e.to_string()),
        };

        let adj = GraphAdjacency::build_from_store(&manager, &device).await?;
        let loss = engine.train_step(&features, &adj, 10.0).await?;

        assert!(loss >= 0.0);
        Ok(())
    }
}
