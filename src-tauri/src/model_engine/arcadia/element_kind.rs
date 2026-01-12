use crate::model_engine::types::ArcadiaElement;
use serde::Serialize;

/// Les 5 couches principales de la méthodologie Arcadia + Data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Layer {
    OperationalAnalysis,  // OA
    SystemAnalysis,       // SA
    LogicalArchitecture,  // LA
    PhysicalArchitecture, // PA
    EPBS,                 // EPBS
    Data,                 // Class, Types
    Unknown,
}

/// Catégorisation fonctionnelle simplifiée des éléments
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum ElementCategory {
    Component,  // System, Logical, Physical Component
    Function,   // Activity, System/Logical/Physical Function
    Actor,      // Operational Actor, System Actor...
    Exchange,   // Functional Exchange, Component Exchange
    Interface,  // Interface, Port
    Data,       // Class, Type
    Capability, // Capability, Scenario
    Other,
}

/// Trait d'extension pour ajouter de l'intelligence sémantique à ArcadiaElement
pub trait ArcadiaSemantics {
    fn get_layer(&self) -> Layer;
    fn get_category(&self) -> ElementCategory;
    fn is_behavioral(&self) -> bool; // Est-ce un élément de comportement (Fonction/Exchange) ?
    fn is_structural(&self) -> bool; // Est-ce un élément de structure (Composant/Interface) ?
}

impl ArcadiaSemantics for ArcadiaElement {
    fn get_layer(&self) -> Layer {
        if self.kind.contains("/oa") || self.kind.contains("#Operational") {
            Layer::OperationalAnalysis
        } else if self.kind.contains("/sa") || self.kind.contains("#System") {
            Layer::SystemAnalysis
        } else if self.kind.contains("/la") || self.kind.contains("#Logical") {
            Layer::LogicalArchitecture
        } else if self.kind.contains("/pa") || self.kind.contains("#Physical") {
            Layer::PhysicalArchitecture
        } else if self.kind.contains("/epbs") || self.kind.contains("#ConfigurationItem") {
            Layer::EPBS
        } else if self.kind.contains("/data") || self.kind.contains("#Data") {
            Layer::Data
        } else {
            Layer::Unknown
        }
    }

    fn get_category(&self) -> ElementCategory {
        let k = &self.kind;
        if k.ends_with("Component") || k.ends_with("System") {
            ElementCategory::Component
        } else if k.ends_with("Function") || k.ends_with("Activity") {
            ElementCategory::Function
        } else if k.ends_with("Actor") {
            ElementCategory::Actor
        } else if k.ends_with("Exchange") || k.ends_with("Flow") {
            ElementCategory::Exchange
        } else if k.ends_with("Interface") || k.ends_with("Port") {
            ElementCategory::Interface
        } else if k.ends_with("Class") || k.ends_with("Type") {
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
    use std::collections::HashMap;

    fn make_el(kind: &str) -> ArcadiaElement {
        ArcadiaElement {
            id: "test".to_string(),
            name: NameType::default(),
            kind: kind.to_string(),
            description: None,
            properties: HashMap::new(),
        }
    }

    #[test]
    fn test_layer_detection() {
        let el_la = make_el("https://arcadia/la#LogicalComponent");
        assert_eq!(el_la.get_layer(), Layer::LogicalArchitecture);

        let el_oa = make_el("https://arcadia/oa#OperationalActivity");
        assert_eq!(el_oa.get_layer(), Layer::OperationalAnalysis);
    }

    #[test]
    fn test_category_detection() {
        let comp = make_el("...#PhysicalComponent");
        assert_eq!(comp.get_category(), ElementCategory::Component);

        let func = make_el("...#SystemFunction");
        assert_eq!(func.get_category(), ElementCategory::Function);
        assert!(func.is_behavioral());
    }
}
