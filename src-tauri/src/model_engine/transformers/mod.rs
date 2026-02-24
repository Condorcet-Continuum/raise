pub mod dialogue_to_model;
pub mod hardware_transformer;
pub mod software_transformer;
pub mod system_transformer;

use crate::utils::prelude::*;

/// Domaine cible de la transformation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformationDomain {
    Software, // Rust, C++, Python -> Classes
    Hardware, // VHDL, Verilog -> Modules
    System,   // Vue d'ensemble -> Docs/Configs
}

/// Trait implémenté par tous les transformateurs de domaine.
/// Il prend un élément brut Arcadia (JSON hydraté) et retourne une structure JSON
/// sémantique optimisée pour le moteur de template.
pub trait ModelTransformer {
    fn transform(&self, element: &Value) -> RaiseResult<Value>;
}

/// Factory pour instancier le bon transformateur
pub fn get_transformer(domain: TransformationDomain) -> Box<dyn ModelTransformer> {
    match domain {
        TransformationDomain::Software => Box::new(software_transformer::SoftwareTransformer),
        TransformationDomain::Hardware => Box::new(hardware_transformer::HardwareTransformer),
        TransformationDomain::System => Box::new(system_transformer::SystemTransformer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_factory_returns_correct_instances() {
        // Test Software
        let soft = get_transformer(TransformationDomain::Software);
        // On vérifie juste que ça ne panic pas et retourne un trait object
        assert!(soft.transform(&serde_json::json!({})).is_ok());

        // Test Hardware
        let hard = get_transformer(TransformationDomain::Hardware);
        assert!(hard.transform(&serde_json::json!({})).is_ok());

        // Test System
        let sys = get_transformer(TransformationDomain::System);
        assert!(sys.transform(&serde_json::json!({})).is_ok());
    }
}
