pub mod common;
pub mod element_kind;

// La macro doit être chargée avant les modules qui l'utilisent
#[macro_use]
pub mod metamodel;

pub mod data;
pub mod epbs;
pub mod logical_architecture;
pub mod operational_analysis;
pub mod physical_architecture;
pub mod system_analysis;

// Re-exports pratiques pour simplifier les imports ailleurs
pub use common::{BaseEntity, ElementRef, I18nString};
pub use element_kind::{ArcadiaSemantics, ElementCategory, Layer};
pub use metamodel::ArcadiaProperties;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modules_integration() {
        // Teste simplement que l'on peut accéder aux types via le mod.rs
        let layer = Layer::OperationalAnalysis;
        assert_eq!(layer, Layer::OperationalAnalysis);

        let i18n = I18nString::default();
        match i18n {
            I18nString::String(s) => assert_eq!(s, ""),
            _ => panic!("Default should be string"),
        }
    }
}
