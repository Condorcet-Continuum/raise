// FICHIER : src-tauri/src/ai/world_model/dynamics/mod.rs

pub mod predictor;

pub use predictor::WorldModelPredictor;

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn test_dynamics_public_api() {
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        // On vérifie qu'on peut instancier le Predictor via l'API publique
        let pred = WorldModelPredictor::new(10, 5, 20, vb);
        assert!(pred.is_ok());
    }
}
