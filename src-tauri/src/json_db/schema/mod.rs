// FICHIER : src-tauri/src/json_db/schema/mod.rs

//! Validation/instanciation de schémas JSON (impl. légère, sans lib externe)

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_enum() {
        // Test basique pour vérifier que l'enum est utilisable
        let err = ValidationError::SchemaNotFound("test".into());
        assert!(matches!(err, ValidationError::SchemaNotFound(_)));
    }
}
