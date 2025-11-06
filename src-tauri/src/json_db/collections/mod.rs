//! Gestionnaire de collections JSON

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub mod manager;
pub mod collection;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection {
    pub name: String,
    pub schema_id: String,
    pub jsonld_context: Option<String>,
    pub indexes: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub id: String,
    pub collection: String,
    pub data: serde_json::Value,
    pub version: u32,
    pub created_at: i64,
    pub updated_at: i64,
}
