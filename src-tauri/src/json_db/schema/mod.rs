//! Validation de schémas JSON Schema

use serde::{Deserialize, Serialize};

pub mod validator;
pub mod registry;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonSchema {
    pub id: String,
    pub title: String,
    pub schema_type: String,
    pub version: String,
    pub schema: serde_json::Value,
    pub created_at: i64,
}

#[derive(Debug)]
pub enum ValidationError {
    SchemaNotFound(String),
    InvalidData(String),
    TypeMismatch(String),
}
