// FICHIER : src-tauri/src/ai/deep_learning/trainer.rs
use crate::utils::prelude::*;

use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use candle_core::Tensor;
use candle_nn::optim::{AdamW, Optimizer, ParamsAdamW};
use candle_nn::VarMap;

/// Gère l'apprentissage du réseau avec l'optimiseur accéléré AdamW.
pub struct Trainer {
    optimizer: AdamW,
}

impl Trainer {
    /// 🎯 Crée un Trainer à partir de la configuration centralisée
    pub fn from_config(varmap: &VarMap, config: &DeepLearningConfig) -> RaiseResult<Self> {
        Self::new(varmap, config.learning_rate)
    }

    /// Constructeur avec paramètres de base (AdamW)
    pub fn new(varmap: &VarMap, learning_rate: f64) -> RaiseResult<Self> {
        let params = ParamsAdamW {
            lr: learning_rate,
            ..Default::default()
        };

        let optimizer = match AdamW::new(varmap.all_vars(), params) {
            Ok(opt) => opt,
            Err(e) => raise_error!("ERR_OPTIMIZER_INIT", error = e.to_string()),
        };

        Ok(Self { optimizer })
    }

    /// Exécute un pas d'entraînement complet : Forward -> Loss -> Backward -> Update (AdamW).
    pub fn train_step(
        &mut self, // 🎯 L'optimiseur a besoin d'être mutable pour garder son Momentum !
        model: &SequenceNet,
        input: &Tensor,
        targets: &Tensor,
    ) -> RaiseResult<f64> {
        // 1. Forward Pass
        let logits = match model.forward(input) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TRAINING_FORWARD", error = e.to_string()),
        };

        // 2. Préparation des dimensions pour la Cross-Entropy
        let (b, s, v) = match logits.dims3() {
            Ok(dims) => dims,
            Err(e) => raise_error!("ERR_TENSOR_DIMS", error = e.to_string()),
        };

        let flat_logits = match logits.reshape((b * s, v)) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TENSOR_RESHAPE", error = e.to_string()),
        };

        let flat_targets = match targets.reshape(b * s) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_TENSOR_RESHAPE", error = e.to_string()),
        };

        // 3. Calcul de la perte avec la fonction native Candle (Ultra optimisée)
        let loss = match candle_nn::loss::cross_entropy(&flat_logits, &flat_targets) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_LOSS_CALC", error = e.to_string()),
        };

        // 4. Backward pass & Weight Update intégrés en une seule passe !
        match self.optimizer.backward_step(&loss) {
            Ok(_) => (),
            Err(e) => raise_error!("ERR_BACKWARD_STEP", error = e.to_string()),
        };

        // 5. Rapatriement du scalaire de loss pour ton monitoring
        match loss.to_scalar::<f32>() {
            Ok(val) => Ok(val as f64),
            Err(e) => raise_error!("ERR_LOSS_SCALAR", error = e.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::testing::DbSandbox;
    use candle_core::DType;
    use candle_nn::VarBuilder;

    #[async_test]
    async fn test_training_convergence() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let config = &sandbox.config.deep_learning;
        let device = config.to_device();

        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        let model = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        )?;

        // 🎯 FIX : Initialisation asynchrone (? car from_config retourne un RaiseResult maintenant)
        // et mot-clé `mut` car le Trainer met à jour son état interne.
        let mut trainer = Trainer::from_config(&varmap, config)?;

        let input = Tensor::randn(0f32, 1.0, (1, 1, config.input_size), &device)?;
        let target = Tensor::zeros((1, 1), DType::U32, &device)?;

        let initial_loss = trainer.train_step(&model, &input, &target)?;

        let mut final_loss = 0.0;
        for _ in 0..20 {
            final_loss = trainer.train_step(&model, &input, &target)?;
        }

        assert!(
            final_loss < initial_loss,
            "Le modèle n'apprend pas avec la config injectée (Loss stable à {})",
            final_loss
        );
        Ok(())
    }
}
