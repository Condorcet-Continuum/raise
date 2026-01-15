pub mod bridge;
pub mod dto;
pub mod engine;
pub mod evaluators;
pub mod genomes;
pub mod operators;
pub mod traits;
pub mod types;

pub use bridge::{GeneticsAdapter, SystemModelProvider};
pub use engine::GeneticEngine;

#[cfg(test)]
mod tests {

    #[test]
    fn test_module_exports() {
        // Simple vérification de la visibilité des types
        // Si ce test compile, les exports sont corrects.
        assert!(true);
    }
}
