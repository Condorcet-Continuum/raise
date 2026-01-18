// FICHIER : src-tauri/src/ai/world_model/perception/mod.rs

// On déclare le sous-module contenant l'implémentation
pub mod encoder;

// On re-exporte la structure principale pour simplifier les imports ailleurs
// (Permet de faire `use crate::ai::world_model::perception::ArcadiaEncoder`)
pub use encoder::ArcadiaEncoder;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::arcadia::element_kind::Layer;

    #[test]
    fn test_perception_public_api() {
        // Ce test vérifie que l'API publique est bien accessible depuis le module
        // C'est un test d'intégration "intra-module".

        // On essaie d'accéder à une fonction de l'encodeur via le re-export
        let result = ArcadiaEncoder::encode_layer(Layer::Data);

        assert!(
            result.is_ok(),
            "L'API ArcadiaEncoder doit être accessible via perception::ArcadiaEncoder"
        );

        let tensor = result.unwrap();
        // Vérif rapide des dimensions (1, 7 pour les Layers)
        assert_eq!(tensor.dims(), &[1, 7]);
    }
}
