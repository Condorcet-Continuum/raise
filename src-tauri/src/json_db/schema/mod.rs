//! Validation/instanciation de schémas JSON (impl. légère, sans lib externe)

pub mod compute;
pub mod registry;
pub use registry::SchemaRegistry;

pub mod validator;
pub use validator::SchemaValidator;

// Optionnel : tu peux garder/étendre ce type pour mapper des erreurs fines
#[derive(Debug)]
pub enum ValidationError {
    SchemaNotFound(String),
    InvalidRef(String),
    InvalidData(String),
    TypeMismatch(String),
    MissingRequired(String),
    AdditionalProperty(String),
    Other(String),
}
