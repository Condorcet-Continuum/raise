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

        // 1. Initialisation de la Couche Cachée (L1)
        let l1 = match linear(input_dim, config.hidden_dim, vb.pp("l1")) {
            Ok(layer) => layer,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_LAYER_INIT_FAILED",
                error = e,
                context = json!({
                    "layer": "l1",
                    "input_dim": input_dim,
                    "output_dim": config.hidden_dim,
                    "hint": "Vérifiez que les poids 'l1' existent dans le VarBuilder et correspondent aux dimensions."
                })
            ),
        };

        // 2. Initialisation de la Couche de Sortie (L2)
        let l2 = match linear(config.hidden_dim, config.embedding_dim, vb.pp("l2")) {
            Ok(layer) => layer,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_LAYER_INIT_FAILED",
                error = e,
                context = json!({
                    "layer": "l2",
                    "input_dim": config.hidden_dim,
                    "output_dim": config.embedding_dim,
                    "hint": "Vérifiez que les poids 'l2' existent dans le VarBuilder."
                })
            ),
        };

        Ok(Self { l1, l2 })
    }

    /// Prédit l'état futur.
    /// * `state` : [Batch, State_Dim]
    /// * `action` : [Batch, Action_Dim]
    pub fn forward(&self, state: &Tensor, action: &Tensor) -> RaiseResult<Tensor> {
        // 1. Concaténation de l'État et de l'Action
        let x = match Tensor::cat(&[state, action], 1) {
            Ok(t) => t,
            Err(e) => raise_error!(
                "ERR_AI_MODEL_CAT_FAILED",
                error = e,
                context = json!({
                    "state_shape": format!("{:?}", state.shape()),
                    "action_shape": format!("{:?}", action.shape()),
                    "dim": 1
                })
            ),
        };

        // 2. Passage dans la couche cachée L1
        let h = match self.l1.forward(&x) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_MODEL_L1_FORWARD_FAILED", error = e),
        };

        // 3. Activation GELU
        let h = match Activation::Gelu.forward(&h) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_MODEL_ACTIVATION_FAILED", error = e),
        };

        // 4. Passage dans la couche de sortie L2
        let next_state = match self.l2.forward(&h) {
            Ok(t) => t,
            Err(e) => raise_error!("ERR_AI_MODEL_L2_FORWARD_FAILED", error = e),
        };

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
