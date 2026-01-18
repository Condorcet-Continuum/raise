// FICHIER : src-tauri/src/ai/world_model/dynamics/predictor.rs

use anyhow::Result;
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
    pub fn new(
        state_dim: usize,
        action_dim: usize,
        hidden_dim: usize,
        vb: VarBuilder,
    ) -> Result<Self> {
        // L'entrée de la couche 1 est la concaténation de State + Action
        let input_dim = state_dim + action_dim;

        let l1 = linear(input_dim, hidden_dim, vb.pp("l1"))?;
        let l2 = linear(hidden_dim, state_dim, vb.pp("l2"))?; // On veut prédire un nouvel état

        Ok(Self { l1, l2 })
    }

    /// Prédit l'état futur.
    /// * `state` : [Batch, State_Dim]
    /// * `action` : [Batch, Action_Dim]
    pub fn forward(&self, state: &Tensor, action: &Tensor) -> Result<Tensor> {
        // 1. Fusion (Early Fusion) : On concatène l'état et l'action
        // dim=1 signifie qu'on colle les colonnes côte à côte
        let x = Tensor::cat(&[state, action], 1)?;

        // 2. Passage dans le réseau de neurones
        let h = self.l1.forward(&x)?;

        // CORRECTION : Utilisation de l'enum Activation pour appliquer GELU
        let h = Activation::Gelu.forward(&h)?;

        let next_state = self.l2.forward(&h)?;

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

        let state_dim = 4;
        let action_dim = 2;
        let hidden_dim = 8;

        let predictor = WorldModelPredictor::new(state_dim, action_dim, hidden_dim, vb).unwrap();

        // Inputs fictifs (Batch = 1)
        let state = Tensor::randn(0f32, 1f32, (1, state_dim), &Device::Cpu).unwrap();
        let action = Tensor::randn(0f32, 1f32, (1, action_dim), &Device::Cpu).unwrap();

        // Forward pass
        let prediction = predictor.forward(&state, &action).unwrap();

        // Vérification : La sortie doit avoir la même forme que l'état (State_Dim)
        assert_eq!(prediction.dims(), &[1, state_dim]);
    }
}
