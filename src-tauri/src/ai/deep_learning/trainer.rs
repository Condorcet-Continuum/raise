use crate::ai::deep_learning::models::sequence_net::SequenceNet;
use candle_core::{Result, Tensor};
use candle_nn::VarMap;

/// Gère l'apprentissage du réseau.
/// Implémente SGD (Stochastic Gradient Descent) manuellement.
pub struct Trainer<'a> {
    varmap: &'a VarMap,
    learning_rate: f64,
}

impl<'a> Trainer<'a> {
    /// Crée un nouveau Trainer.
    ///
    /// # Arguments
    /// * `varmap` - Référence vers la VarMap contenant tous les poids du modèle.
    /// * `learning_rate` - Taux d'apprentissage (ex: 0.01).
    pub fn new(varmap: &'a VarMap, learning_rate: f64) -> Self {
        Self {
            varmap,
            learning_rate,
        }
    }

    /// Exécute un pas d'entraînement complet : Forward -> Loss -> Backward -> Update.
    ///
    /// # Arguments
    /// * `model` - Le modèle à entraîner.
    /// * `input` - Tenseur d'entrée [batch, seq_len, input_dim].
    /// * `targets` - Tenseur d'indices cibles [batch, seq_len] (classes attendues).
    ///
    /// # Retourne
    /// La valeur de la perte (Loss) pour ce pas.
    pub fn train_step(&self, model: &SequenceNet, input: &Tensor, targets: &Tensor) -> Result<f64> {
        // 1. Forward Pass : Calcul des prédictions
        let logits = model.forward(input)?; // [batch, seq, output_dim]

        // 2. Calcul de la Loss (Cross Entropy)
        let loss = self.cross_entropy_loss(&logits, targets)?;

        // 3. Backward Pass : Calcul des gradients
        let grads = loss.backward()?;

        // 4. Optimization Step (SGD manuel)
        // w = w - learning_rate * grad
        for var in self.varmap.all_vars() {
            if let Some(grad) = grads.get(&var) {
                let lr = self.learning_rate;

                // CORRECTION : Conversion explicite en f32 pour correspondre au type des poids (F32)
                let lr_tensor = Tensor::new(lr as f32, grad.device())?;

                // Calcul du delta : grad * lr (avec broadcast)
                let delta = grad.broadcast_mul(&lr_tensor)?;

                // Mise à jour : var - delta
                let new_val = var.as_tensor().sub(&delta)?;
                var.set(&new_val)?;
            }
        }

        // On retourne la valeur scalaire de la loss pour le monitoring
        loss.to_scalar::<f32>().map(|v| v as f64)
    }

    /// Implémentation robuste de Cross Entropy Loss
    /// L = -mean(log_softmax(logits)[targets])
    fn cross_entropy_loss(&self, logits: &Tensor, targets: &Tensor) -> Result<Tensor> {
        let (b, s, v) = logits.dims3()?;

        // Aplatir les batchs et séquences pour traiter chaque token indépendamment
        // Logits : [N, Vocab] où N = batch * seq
        let flat_logits = logits.reshape((b * s, v))?;
        // Cibles : [N]
        let flat_targets = targets.reshape(b * s)?;

        // 1. Log Softmax sur la dimension du vocabulaire
        let log_probs = candle_nn::ops::log_softmax(&flat_logits, 1)?;

        // 2. Sélectionner la log-probabilité correspondant à la vraie classe (Gather)
        let target_indexes = flat_targets.unsqueeze(1)?;
        let selected_log_probs = log_probs.gather(&target_indexes, 1)?;

        // 3. Moyenne négative (Negative Log Likelihood)
        let loss = selected_log_probs.mean_all()?.neg()?;

        Ok(loss)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarBuilder;

    #[test]
    fn test_training_convergence() -> Result<()> {
        let device = Device::Cpu;
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &device);

        // Configuration simple
        let input_dim = 4;
        let hidden_dim = 8;
        let output_dim = 2;
        let model = SequenceNet::new(input_dim, hidden_dim, output_dim, vb)?;

        let trainer = Trainer::new(&varmap, 0.1);

        // Données constantes
        let input = Tensor::randn(0f32, 1.0, (1, 1, input_dim), &device)?;
        let target = Tensor::zeros((1, 1), DType::U32, &device)?;

        // Mesure initiale
        let initial_loss = trainer.train_step(&model, &input, &target)?;
        println!("Initial Loss: {}", initial_loss);

        // Entraînement
        let mut final_loss = 0.0;
        for _ in 0..20 {
            final_loss = trainer.train_step(&model, &input, &target)?;
        }
        println!("Final Loss: {}", final_loss);

        // Vérification
        assert!(
            final_loss < initial_loss,
            "Le modèle n'apprend pas (Loss ne descend pas)"
        );
        Ok(())
    }
}
