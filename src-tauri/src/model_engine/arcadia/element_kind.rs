// FICHIER : src-tauri/src/model_engine/arcadia/element_kind.rs

use crate::json_db::jsonld::vocabulary::namespaces; // Import des namespaces officiels
use crate::model_engine::types::ArcadiaElement;
use serde::Serialize;

/// Les couches principales de la méthodologie Arcadia + Data + Transverse
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Layer {
    OperationalAnalysis,  // OA
    SystemAnalysis,       // SA
    LogicalArchitecture,  // LA
    PhysicalArchitecture, // PA
    EPBS,                 // EPBS
    Data,                 // Class, Types
    Transverse,           // Common, Libraries, Shared definitions
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
        // Détection robuste basée sur les préfixes d'URI définis dans l'ontologie
        // Grâce au Loader, self.kind est garanti être une URI complète (ou normalisée).

        if self.kind.starts_with(namespaces::OA) {
            Layer::OperationalAnalysis
        } else if self.kind.starts_with(namespaces::SA) {
            Layer::SystemAnalysis
        } else if self.kind.starts_with(namespaces::LA) {
            Layer::LogicalArchitecture
        } else if self.kind.starts_with(namespaces::PA) {
            Layer::PhysicalArchitecture
        } else if self.kind.starts_with(namespaces::EPBS) {
            Layer::EPBS
        } else if self.kind.starts_with(namespaces::DATA) {
            Layer::Data
        }
        // Détection de la couche Transverse (souvent /transverse ou /common ou /libraries)
        // Comme il n'y a pas encore de namespace::TRANSVERSE officiel, on vérifie le segment d'URI
        else if self.kind.contains("/transverse")
            || self.kind.contains("/common")
            || self.kind.contains("/libraries")
        {
            Layer::Transverse
        } else {
            // Fallback pour compatibilité partielle ou types externes
            Layer::Unknown
        }
    }

    fn get_category(&self) -> ElementCategory {
        let k = &self.kind;

        // Note: Avec des URIs complètes (ex: ...#SystemComponent), ends_with fonctionne parfaitement
        // car le fragment (#...) est toujours à la fin.

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
    fn test_layer_detection_with_namespaces() {
        // Test avec les vraies URIs de production construites via le vocabulaire
        let el_oa = make_el(&format!("{}{}", namespaces::OA, "OperationalActor"));
        assert_eq!(el_oa.get_layer(), Layer::OperationalAnalysis);

        let el_sa = make_el(&format!("{}{}", namespaces::SA, "SystemFunction"));
        assert_eq!(el_sa.get_layer(), Layer::SystemAnalysis);

        let el_data = make_el(&format!("{}{}", namespaces::DATA, "Class"));
        assert_eq!(el_data.get_layer(), Layer::Data);

        // Test Transverse
        let el_trans = make_el("https://raise.io/ontology/arcadia/transverse#CommonLib");
        assert_eq!(el_trans.get_layer(), Layer::Transverse);

        // Test cas inconnu
        let el_unknown = make_el("http://unknown.org/thing");
        assert_eq!(el_unknown.get_layer(), Layer::Unknown);
    }

    #[test]
    fn test_category_detection() {
        // Test structurel (PA)
        let comp = make_el(&format!("{}{}", namespaces::PA, "PhysicalComponent"));
        assert_eq!(comp.get_category(), ElementCategory::Component);
        assert!(comp.is_structural());

        // Test comportemental (SA)
        let func = make_el(&format!("{}{}", namespaces::SA, "SystemFunction"));
        assert_eq!(func.get_category(), ElementCategory::Function);
        assert!(func.is_behavioral());

        // Test Data
        let data = make_el(&format!("{}{}", namespaces::DATA, "DataType"));
        assert_eq!(data.get_category(), ElementCategory::Data);
    }
}
