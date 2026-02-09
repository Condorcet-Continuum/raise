// FICHIER : src-tauri/src/json_db/query/mod.rs

pub mod executor;
pub mod optimizer;
pub mod parser;
pub mod sql;

use crate::utils::json::{Deserialize, Serialize, Value};

pub use executor::QueryEngine;

// --- Structures de Données ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Query {
    pub collection: String,
    pub filter: Option<QueryFilter>,
    pub sort: Option<Vec<SortField>>,
    pub limit: Option<usize>,
    pub offset: Option<usize>,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Condition {
    pub field: String,
    pub operator: ComparisonOperator,
    pub value: Value,
}

// --- IMPLÉMENTATION DES HELPERS (CORRECTION) ---
impl Condition {
    pub fn new(field: impl Into<String>, operator: ComparisonOperator, value: Value) -> Self {
        Self {
            field: field.into(),
            operator,
            value,
        }
    }

    pub fn eq(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Eq, value)
    }

    pub fn ne(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Ne, value)
    }

    pub fn gt(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Gt, value)
    }

    pub fn gte(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Gte, value)
    }

    pub fn lt(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Lt, value)
    }

    pub fn lte(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Lte, value)
    }

    pub fn contains(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Contains, value)
    }

    pub fn starts_with(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::StartsWith, value)
    }

    pub fn ends_with(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::EndsWith, value)
    }

    pub fn r#in(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::In, value)
    }

    pub fn matches(field: impl Into<String>, value: Value) -> Self {
        Self::new(field, ComparisonOperator::Matches, value)
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
    Matches, // Regex
    Like,    // SQL Like
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
    use crate::utils::json::{self, json};

    #[test]
    fn test_condition_helpers() {
        let c = Condition::gt("age", json!(18));
        assert_eq!(c.operator, ComparisonOperator::Gt);
        assert_eq!(c.field, "age");
    }

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

        let json_str = json::stringify(&query).unwrap();
        assert!(json_str.contains("\"users\""));
        assert!(json_str.contains("\"age\""));
        assert!(json_str.contains("\"Include\""));
    }
}
