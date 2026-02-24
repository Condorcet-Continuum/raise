// FICHIER : src-tauri/src/ai/world_model/dynamics/predictor.rs

use crate::utils::config::WorldModelConfig;
use crate::utils::prelude::*;
use candle_core::{Module, Tensor};
// On garde Activation car on va l'utiliser
use candle_nn::{linear, Activation, Linear, VarBuilder};

/// Le Prédicteur de Transition (World Model Dynamics).
/// Il apprend la fonction : f(État_t, Action_t) -> État_t+1
pub struct WorldModelPredictor {
    /// Première couche : Combine État + Action -> Caché
    l1: Linear,
    /// Seconde couche : Caché -> État Futur prédit
    l2: Linear,
}

impl WorldModelPredictor {
    /// Crée un nouveau prédicteur.
    /// * `state_dim` : Dimension du vecteur d'état (Latent).
    /// * `action_dim` : Dimension du vecteur d'action (ex: 10 pour 10 types de commandes).
    /// * `hidden_dim` : Taille de la couche cachée (Neurones).
    pub fn new(config: &WorldModelConfig, vb: VarBuilder) -> RaiseResult<Self> {
        // L'entrée de la couche 1 est la concaténation de State + Action
        let input_dim = config.embedding_dim + config.action_dim;

        let l1 = linear(input_dim, config.hidden_dim, vb.pp("l1"))
            .map_err(|e| AppError::from(e.to_string()))?;

        let l2 = linear(config.hidden_dim, config.embedding_dim, vb.pp("l2"))
            .map_err(|e| AppError::from(e.to_string()))?;

        Ok(Self { l1, l2 })
    }

    /// Prédit l'état futur.
    /// * `state` : [Batch, State_Dim]
    /// * `action` : [Batch, Action_Dim]
    pub fn forward(&self, state: &Tensor, action: &Tensor) -> RaiseResult<Tensor> {
        // ✅ Conversion des erreurs pour chaque opération de tenseur
        let x = Tensor::cat(&[state, action], 1).map_err(|e| AppError::from(e.to_string()))?;

        let h = self
            .l1
            .forward(&x)
            .map_err(|e| AppError::from(e.to_string()))?;

        let h = Activation::Gelu
            .forward(&h)
            .map_err(|e| AppError::from(e.to_string()))?;

        let next_state = self
            .l2
            .forward(&h)
            .map_err(|e| AppError::from(e.to_string()))?;
        Ok(next_state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::VarMap;

    #[test]
    fn test_predictor_shape() {
        // Setup
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        // 🎯 Création d'une config sur-mesure pour le test
        let config = WorldModelConfig {
            vocab_size: 10,
            embedding_dim: 4,
            action_dim: 2,
            hidden_dim: 8,
            use_gpu: false,
        };

        let predictor = WorldModelPredictor::new(&config, vb).unwrap();

        // Inputs fictifs (Batch = 1)
        let state = Tensor::randn(0f32, 1f32, (1, config.embedding_dim), &Device::Cpu).unwrap();
        let action = Tensor::randn(0f32, 1f32, (1, config.action_dim), &Device::Cpu).unwrap();

        // Forward pass
        let prediction = predictor.forward(&state, &action).unwrap();

        // Vérification : La sortie doit avoir la même forme que l'état (State_Dim)
        assert_eq!(prediction.dims(), &[1, config.embedding_dim]);
    }
}
