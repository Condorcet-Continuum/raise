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
    /// Liste des champs à inclure/exclure. Si None -> SELECT *
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Projection {
    Include(Vec<String>),
    Exclude(Vec<String>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryFilter {
    pub operator: FilterOperator,
    pub conditions: Vec<Condition>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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
    pub fn eq(field: impl Into<String>, value: Value) -> Self {
        Self {
            field: field.into(),
            operator: ComparisonOperator::Eq,
            value,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
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

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_query_serialization() {
        let query = Query {
            collection: "users".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("age", json!(18))],
            }),
            sort: None,
            limit: Some(10),
            offset: None,
            projection: Some(Projection::Include(vec!["name".into()])),
        };

        let json = serde_json::to_string(&query).unwrap();
        assert!(json.contains("\"users\""));
        assert!(json.contains("\"age\""));
        assert!(json.contains("\"Include\""));
    }
}
