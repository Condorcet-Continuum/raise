// FICHIER : src-tauri/src/json_db/query/mod.rs

pub mod executor;
pub mod optimizer;
pub mod parser;
pub mod sql;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub use executor::QueryEngine;

// --- Structures de Données ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub collection: String,
    pub filter: Option<QueryFilter>,
    pub sort: Option<Vec<SortField>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
    /// Liste des champs à inclure. Si None ou vide -> SELECT *
    pub projection: Option<Projection>,
}

impl Query {
    pub fn new(collection: &str) -> Self {
        Self {
            collection: collection.to_string(),
            filter: None,
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        }
    }
}

// Nouvelle Enum pour gérer proprement les projections (SELECT a, b)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Projection {
    Include(Vec<String>),
    Exclude(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub operator: FilterOperator,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterOperator {
    And,
    Or,
    Not,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Condition {
    pub field: String,
    pub operator: ComparisonOperator,
    pub value: Value,
}

impl Condition {
    // Helper indispensable pour le parser et les tests
    pub fn eq(field: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            operator: ComparisonOperator::Eq,
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ComparisonOperator {
    Eq,
    Ne,
    Gt,
    Gte,
    Lt,
    Lte,
    In,
    Contains,
    StartsWith,
    EndsWith,
    Matches,
    Like,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SortField {
    pub field: String,
    pub order: SortOrder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SortOrder {
    Asc,
    Desc,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct QueryResult {
    pub documents: Vec<Value>,
    pub total_count: u64,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
