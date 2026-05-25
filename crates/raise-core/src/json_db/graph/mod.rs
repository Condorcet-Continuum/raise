// FICHIER : src-tauri/src/json_db/graph/mod.rs

pub mod semantic_manager;

// Re-export pour faciliter l'utilisation depuis l'extérieur (ex: raise::json_db::graph::SemanticManager)
pub use semantic_manager::SemanticManager;
