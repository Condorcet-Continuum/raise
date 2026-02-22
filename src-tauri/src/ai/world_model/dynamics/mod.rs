// FICHIER : src-tauri/src/ai/world_model/dynamics/mod.rs

pub mod predictor;

pub use predictor::WorldModelPredictor;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::config::WorldModelConfig;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn test_predictor_instantiation() {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        // 🎯 On crée la config
        let config = WorldModelConfig {
            embedding_dim: 16,
            action_dim: 5,
            hidden_dim: 32,
            ..Default::default()
        };

        // 🎯 On passe la config par référence
        let _predictor = predictor::WorldModelPredictor::new(&config, vb).unwrap();
        assert!(true);
    }
}
