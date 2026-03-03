// FICHIER : src-tauri/src/json_db/schema/mod.rs

//! Validation/instanciation de schémas JSON (impl. légère, sans lib externe)

pub mod registry;
pub use registry::SchemaRegistry;

pub mod validator;
pub use validator::SchemaValidator;
