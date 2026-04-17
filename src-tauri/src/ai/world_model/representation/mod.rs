// FICHIER : src-tauri/src/ai/world_model/representation/mod.rs

// On déclare le module qui contient la logique VQ
pub mod quantizer;

// On re-exporte la struct pour qu'elle soit accessible via representation::VectorQuantizer
pub use quantizer::VectorQuantizer;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::prelude::*;

    #[test]
    fn test_representation_public_api() {
        let varmap = NeuralWeightsMap::new();
        let vb =
            NeuralWeightsBuilder::from_varmap(&varmap, ComputeType::F32, &ComputeHardware::Cpu);

        // 🎯 FIX : On crée une config locale pour le test
        let config = WorldModelConfig {
            vocab_size: 10,
            embedding_dim: 4,
            action_dim: 5,
            hidden_dim: 32,
            use_gpu: false,
        };

        // 🎯 FIX : On passe la référence à la config
        let vq = VectorQuantizer::new(&config, vb);
        assert!(vq.is_ok(), "L'API VectorQuantizer doit être accessible.");
    }
}
