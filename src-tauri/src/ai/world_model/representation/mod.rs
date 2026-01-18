// FICHIER : src-tauri/src/ai/world_model/representation/mod.rs

// On déclare le module qui contient la logique VQ
pub mod quantizer;

// On re-exporte la struct pour qu'elle soit accessible via representation::VectorQuantizer
pub use quantizer::VectorQuantizer;

#[cfg(test)]
mod tests {
    use super::*;
    use candle_core::{DType, Device};
    use candle_nn::{VarBuilder, VarMap};

    #[test]
    fn test_representation_public_api() {
        // Test d'intégration intra-module
        // Vérifie qu'on peut instancier le Quantizer depuis l'extérieur du fichier
        let varmap = VarMap::new();
        let vb = VarBuilder::from_varmap(&varmap, DType::F32, &Device::Cpu);

        let vq = VectorQuantizer::new(10, 4, vb);
        assert!(vq.is_ok(), "L'API VectorQuantizer doit être accessible.");
    }
}
