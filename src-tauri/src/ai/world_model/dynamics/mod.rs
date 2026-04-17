// FICHIER : src-tauri/src/ai/world_model/dynamics/mod.rs

pub mod predictor;

pub use predictor::WorldModelPredictor;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::prelude::*;

    #[test]
    #[serial_test::serial] // Sécurité : L'orchestrateur charge l'IA
    #[cfg_attr(not(feature = "cuda"), ignore)]
    fn test_predictor_instantiation() {
        let varmap = NeuralWeightsMap::new();
        let vb =
            NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, &ComputeHardware::Cpu);

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
