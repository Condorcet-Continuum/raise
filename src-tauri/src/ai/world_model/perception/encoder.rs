// FICHIER : src-tauri/src/ai/world_model/perception/encoder.rs

use crate::utils::prelude::*;

use crate::model_engine::arcadia::element_kind::{ArcadiaSemantics, ElementCategory, Layer};
use crate::model_engine::types::ArcadiaElement;
use candle_core::{Device, Tensor};

/// Dimensions fixes pour l'encodage One-Hot
/// OA, SA, LA, PA, EPBS, Data, Transverse, Unknown -> 8 dimensions
const LAYER_DIM: usize = 8;
/// Component, Function, Actor, Exchange, Interface, Data, Capability, Other -> 8 dimensions
const CATEGORY_DIM: usize = 8;

/// Encodeur sans état (Stateless) pour transformer les concepts Arcadia en Tenseurs.
pub struct ArcadiaEncoder;

impl ArcadiaEncoder {
    /// Encode la couche (Layer) en vecteur One-Hot [1, 8]
    pub fn encode_layer(layer: Layer) -> Result<Tensor> {
        let index = match layer {
            Layer::OperationalAnalysis => 0,
            Layer::SystemAnalysis => 1,
            Layer::LogicalArchitecture => 2,
            Layer::PhysicalArchitecture => 3,
            Layer::EPBS => 4,
            Layer::Data => 5,
            Layer::Transverse => 6, // AJOUT
            Layer::Unknown => 7,
        };

        Self::one_hot(index, LAYER_DIM)
    }

    /// Encode la catégorie fonctionnelle en vecteur One-Hot [1, 8]
    pub fn encode_category(category: ElementCategory) -> Result<Tensor> {
        let index = match category {
            ElementCategory::Component => 0,
            ElementCategory::Function => 1,
            ElementCategory::Actor => 2,
            ElementCategory::Exchange => 3,
            ElementCategory::Interface => 4,
            ElementCategory::Data => 5,
            ElementCategory::Capability => 6,
            ElementCategory::Other => 7,
        };

        Self::one_hot(index, CATEGORY_DIM)
    }

    /// Encode un élément complet (Concaténation Layer + Category)
    /// Dimension de sortie : [1, 16] (8 + 8)
    pub fn encode_element(element: &ArcadiaElement) -> Result<Tensor> {
        // 1. Extraction sémantique via le Trait existant
        let layer = element.get_layer();
        let category = element.get_category();

        // 2. Encodage individuel
        let t_layer = Self::encode_layer(layer)?;
        let t_cat = Self::encode_category(category)?;

        // 3. Concaténation (Feature Fusion)
        // On fusionne sur la dimension 1 (les features)
        let t_combined =
            Tensor::cat(&[&t_layer, &t_cat], 1).map_err(|e| AppError::from(e.to_string()))?;
        Ok(t_combined)
    }

    /// Helper pour générer un vecteur One-Hot
    fn one_hot(index: usize, size: usize) -> Result<Tensor> {
        let mut data = vec![0f32; size];
        if index < size {
            data[index] = 1.0;
        }
        // Création du tenseur sur CPU (Device::Cpu est par défaut safe)
        Tensor::from_vec(data, (1, size), &Device::Cpu).map_err(|e| AppError::from(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::utils::HashMap;

    // Helper pour créer un élément dummy
    fn make_element(kind: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: "test_id".to_string(),
            name: NameType::default(),
            kind: kind.to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_encode_layer_sa() {
        // SA est l'index 1 -> [0, 1, 0, 0, 0, 0, 0, 0]
        let t = ArcadiaEncoder::encode_layer(Layer::SystemAnalysis).unwrap();
        let vec: Vec<f32> = t.to_vec2::<f32>().unwrap()[0].clone();

        assert_eq!(vec.len(), LAYER_DIM);
        assert_eq!(vec[1], 1.0);
        assert_eq!(vec[0], 0.0);
    }

    #[test]
    fn test_encode_category_function() {
        // Function est l'index 1 -> [0, 1, 0, 0, 0, 0, 0, 0]
        let t = ArcadiaEncoder::encode_category(ElementCategory::Function).unwrap();
        let vec: Vec<f32> = t.to_vec2::<f32>().unwrap()[0].clone();

        assert_eq!(vec.len(), CATEGORY_DIM);
        assert_eq!(vec[1], 1.0);
    }

    #[test]
    fn test_encode_full_element() {
        // Un LogicalComponent dans LA
        // Layer LA = index 2
        // Category Component = index 0
        // NOTE: On utilise une URI valide pour passer la validation stricte de element_kind.rs
        let el = make_element("https://raise.io/ontology/arcadia/la#LogicalComponent");

        let t = ArcadiaEncoder::encode_element(&el).unwrap();
        let vec: Vec<f32> = t.to_vec2::<f32>().unwrap()[0].clone();

        // Taille totale attendue : 8 + 8 = 16
        assert_eq!(vec.len(), LAYER_DIM + CATEGORY_DIM);

        // Vérif Layer part (index 2)
        assert_eq!(vec[2], 1.0, "Layer index 2 (LA) doit être 1.0");

        // Vérif Category part.
        // LAYER_DIM est 8.
        // Category Component est index 0 localement.
        // Index global = 8 + 0 = 8.
        assert_eq!(
            vec[8], 1.0,
            "Category index 0 (Component) décalé de 8 doit être 1.0"
        );
    }
}
