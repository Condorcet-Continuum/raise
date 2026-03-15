// FICHIER : src-tauri/src/model_engine/arcadia/element_kind.rs

use crate::model_engine::types::ArcadiaElement;
use crate::utils::prelude::*;

/// Les couches principales de la méthodologie Arcadia + Data + Transverse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serializable)]
pub enum Layer {
    OperationalAnalysis,
    SystemAnalysis,
    LogicalArchitecture,
    PhysicalArchitecture,
    EPBS,
    Data,
    Transverse,
    Unknown,
}

/// Catégorisation fonctionnelle simplifiée des éléments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serializable)]
pub enum ElementCategory {
    Component,
    Function,
    Actor,
    Exchange,
    Interface,
    Data,
    Capability,
    Other,
}

impl Layer {
    pub const COUNT: usize = 8;
    pub fn index(&self) -> usize {
        *self as usize
    }
}

impl ElementCategory {
    pub const COUNT: usize = 8;
    pub fn index(&self) -> usize {
        *self as usize
    }
}

/// Trait d'extension pour ajouter de l'intelligence sémantique à ArcadiaElement
pub trait ArcadiaSemantics {
    fn get_layer(&self) -> Layer;
    fn get_category(&self) -> ElementCategory;
    fn is_behavioral(&self) -> bool;
    fn is_structural(&self) -> bool;
}

impl ArcadiaSemantics for ArcadiaElement {
    fn get_layer(&self) -> Layer {
        let k = &self.kind;

        // Déduction agnostique par segment d'URI
        if k.contains("/oa#") {
            Layer::OperationalAnalysis
        } else if k.contains("/sa#") {
            Layer::SystemAnalysis
        } else if k.contains("/la#") {
            Layer::LogicalArchitecture
        } else if k.contains("/pa#") {
            Layer::PhysicalArchitecture
        } else if k.contains("/epbs#") {
            Layer::EPBS
        } else if k.contains("/data#") {
            Layer::Data
        } else if k.contains("/transverse") || k.contains("/common") || k.contains("/libraries") {
            Layer::Transverse
        } else {
            Layer::Unknown
        }
    }

    fn get_category(&self) -> ElementCategory {
        let k = &self.kind;

        // Déduction agnostique par suffixe d'URI
        if k.ends_with("Component") || k.ends_with("System") || k.ends_with("ConfigurationItem") {
            ElementCategory::Component
        } else if k.ends_with("Function") || k.ends_with("Activity") {
            ElementCategory::Function
        } else if k.ends_with("Actor") {
            ElementCategory::Actor
        } else if k.ends_with("Exchange") || k.ends_with("Flow") || k.ends_with("Link") {
            ElementCategory::Exchange
        } else if k.ends_with("Interface") || k.ends_with("Port") {
            ElementCategory::Interface
        } else if k.ends_with("Class") || k.ends_with("DataType") || k.ends_with("ExchangeItem") {
            ElementCategory::Data
        } else if k.ends_with("Capability") || k.ends_with("Scenario") {
            ElementCategory::Capability
        } else {
            ElementCategory::Other
        }
    }

    fn is_behavioral(&self) -> bool {
        matches!(
            self.get_category(),
            ElementCategory::Function | ElementCategory::Exchange | ElementCategory::Capability
        )
    }

    fn is_structural(&self) -> bool {
        matches!(
            self.get_category(),
            ElementCategory::Component | ElementCategory::Interface | ElementCategory::Actor
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_engine::types::{ArcadiaElement, NameType};
    use crate::utils::data::UnorderedMap;

    fn make_el(kind: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: "test".to_string(),
            name: NameType::default(),
            kind: kind.to_string(),
            description: None,
            properties: UnorderedMap::new(),
        }
    }

    #[test]
    fn test_layer_detection_agnostic() {
        let el_oa = make_el("https://raise.io/ontology/arcadia/oa#OperationalActor");
        assert_eq!(el_oa.get_layer(), Layer::OperationalAnalysis);

        let el_sa = make_el("https://raise.io/ontology/arcadia/sa#SystemFunction");
        assert_eq!(el_sa.get_layer(), Layer::SystemAnalysis);

        let el_data = make_el("https://raise.io/ontology/arcadia/data#Class");
        assert_eq!(el_data.get_layer(), Layer::Data);

        let el_trans = make_el("https://raise.io/ontology/arcadia/transverse#CommonLib");
        assert_eq!(el_trans.get_layer(), Layer::Transverse);

        let el_unknown = make_el("http://unknown.org/thing");
        assert_eq!(el_unknown.get_layer(), Layer::Unknown);
    }

    #[test]
    fn test_category_detection_agnostic() {
        let comp = make_el("https://raise.io/ontology/arcadia/pa#PhysicalComponent");
        assert_eq!(comp.get_category(), ElementCategory::Component);
        assert!(comp.is_structural());

        let func = make_el("https://raise.io/ontology/arcadia/sa#SystemFunction");
        assert_eq!(func.get_category(), ElementCategory::Function);
        assert!(func.is_behavioral());

        let data = make_el("https://raise.io/ontology/arcadia/data#DataType");
        assert_eq!(data.get_category(), ElementCategory::Data);
    }
}
