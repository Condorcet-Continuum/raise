// FICHIER : src-tauri/src/ai/world_model/mod.rs

pub mod dynamics;
pub mod engine;
pub mod perception;
pub mod representation;
pub mod training;

pub use engine::{NeuroSymbolicEngine, WorldAction};
pub use training::WorldTrainer; // <--- AJOUT pour accès facile

#[cfg(test)]
mod tests {
    #[test]
    fn test_world_model_structure() {
        assert!(true);
    }
}
