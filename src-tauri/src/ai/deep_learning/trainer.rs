// FICHIER : src-tauri/src/ai/deep_learning/trainer.rs

use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use crate::utils::config::DeepLearningConfig; // 🎯 Nouvel import
use candle_core::{Result, Tensor};
use candle_nn::VarMap;

/// Gère l'apprentissage du réseau.
pub struct Trainer<'a> {
    varmap: &'a VarMap,
    learning_rate: f64,
}

impl<'a> Trainer<'a> {
    /// 🎯 NOUVEAU : Crée un Trainer à partir de la configuration centralisée
    pub fn from_config(varmap: &'a VarMap, config: &DeepLearningConfig) -> Self {
        Self {
            varmap,
            learning_rate: config.learning_rate,
        }
    }

    /// Constructeur classique (conservé pour la flexibilité)
    pub fn new(varmap: &'a VarMap, learning_rate: f64) -> Self {
        Self {
            varmap,
            learning_rate,
        }
    }

    /// Exécute un pas d'entraînement complet : Forward -> Loss -> Backward -> Update.
    pub fn train_step(&self, model: &SequenceNet, input: &Tensor, targets: &Tensor) -> Result<f64> {
        let logits = model.forward(input)?;
        let loss = self.cross_entropy_loss(&logits, targets)?;
        let grads = loss.backward()?;

        for var in self.varmap.all_vars() {
            if let Some(grad) = grads.get(&var) {
                let lr_tensor = Tensor::new(self.learning_rate as f32, grad.device())?;
                let delta = grad.broadcast_mul(&lr_tensor)?;
                let new_val = var.as_tensor().sub(&delta)?;
                var.set(&new_val)?;
            }
        }

        loss.to_scalar::<f32>().map(|v| v as f64)
    }

    fn cross_entropy_loss(&self, logits: &Tensor, targets: &Tensor) -> Result<Tensor> {
        let (b, s, v) = logits.dims3()?;
        let flat_logits = logits.reshape((b * s, v))?;
        let flat_targets = targets.reshape(b * s)?;

        let log_probs = candle_nn::ops::log_softmax(&flat_logits, 1)?;
        let target_indexes = flat_targets.unsqueeze(1)?;
        let selected_log_probs = log_probs.gather(&target_indexes, 1)?;

        selected_log_probs.mean_all()?.neg()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::{test_mocks, AppConfig}; // 🎯 Import des mocks
    use candle_core::DType;
    use candle_nn::VarBuilder;

    #[test]
    fn test_training_convergence() -> Result<()> {
        // 1. Initialisation via le Singleton (Moule de test : 10, 20, 5)
        test_mocks::inject_mock_config();
        let config = &AppConfig::get().deep_learning;
        let device = config.to_device();

        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        // 🎯 2. Utilisation des dimensions du Singleton (Alignement SSOT)
        let model = SequenceNet::new(
            config.input_size,
            config.hidden_size,
            config.output_size,
            vb,
        )?;

        // On utilise le constructeur intelligent
        let trainer = Trainer::from_config(&varmap, config);

        // Données d'entrée alignées sur la config (input_size)
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
