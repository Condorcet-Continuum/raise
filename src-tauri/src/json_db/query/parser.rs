// FICHIER : src-tauri/src/json_db/query/parser.rs

use anyhow::{bail, Result};
use serde_json::Value;

use super::{
    ComparisonOperator, Condition, FilterOperator, Projection, Query, QueryFilter, SortField,
    SortOrder,
};

// Parsing des projections (champs "name" ou "-password")
pub fn parse_projection(fields: &[String]) -> Result<Projection> {
    if fields.is_empty() {
        bail!("Empty projection");
    }

    let is_exclude = fields[0].starts_with('-');
    let cleaned: Vec<String> = fields
        .iter()
        .map(|f| f.trim_start_matches(['+', '-']).to_string())
        .collect();

    if is_exclude {
        Ok(Projection::Exclude(cleaned))
    } else {
        Ok(Projection::Include(cleaned))
    }
}

// Builder Fluent pour construire des requêtes
pub struct QueryBuilder {
    query: Query,
}

impl QueryBuilder {
    pub fn new(collection: impl Into<String>) -> Self {
        let col_str: String = collection.into();
        Self {
            query: Query::new(&col_str),
        }
    }

    pub fn where_eq(mut self, field: impl Into<String>, value: Value) -> Self {
        let c = Condition::eq(field, value);
        self.add_cond(FilterOperator::And, c);
        self
    }

    fn add_cond(&mut self, op: FilterOperator, c: Condition) {
        if let Some(ref mut f) = self.query.filter {
            f.conditions.push(c);
        } else {
            self.query.filter = Some(QueryFilter {
                operator: op,
                conditions: vec![c],
            });
        }
    }

    pub fn select(mut self, fields: Vec<String>) -> Result<Self> {
        self.query.projection = Some(parse_projection(&fields)?);
        Ok(self)
    }

    pub fn build(self) -> Query {
        self.query
    }
}

pub fn parse_sort_specs(specs: &[String]) -> Result<Vec<SortField>> {
    let mut out = Vec::new();
    for spec in specs {
        out.push(parse_single_sort_spec(spec)?);
    }
    Ok(out)
}

fn parse_single_sort_spec(spec: &str) -> Result<SortField> {
    let spec = spec.trim();
    if let Some(f) = spec.strip_prefix('+') {
        return Ok(SortField {
            field: f.trim().to_string(),
            order: SortOrder::Asc,
        });
    }
    if let Some(f) = spec.strip_prefix('-') {
        return Ok(SortField {
            field: f.trim().to_string(),
            order: SortOrder::Desc,
        });
    }

    let (field, order) = match spec.split_once(':') {
        Some((f, o)) => (
            f.trim(),
            match o.trim().to_lowercase().as_str() {
                "desc" | "descending" => SortOrder::Desc,
                _ => SortOrder::Asc,
            },
        ),
        None => (spec, SortOrder::Asc),
    };
    Ok(SortField {
        field: field.to_string(),
        order,
    })
}

pub fn parse_filter_from_json(value: &Value) -> Result<QueryFilter> {
    let obj = value
        .as_object()
        .ok_or_else(|| anyhow::anyhow!("Not an object"))?;
    let op = match obj
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("and")
    {
        "or" => FilterOperator::Or,
        "not" => FilterOperator::Not,
        _ => FilterOperator::And,
    };

    let mut conditions = Vec::new();
    if let Some(arr) = obj.get("conditions").and_then(|v| v.as_array()) {
        for c in arr {
            if let Some(co) = c.as_object() {
                let f = co
                    .get("field")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let o_str = co.get("operator").and_then(|v| v.as_str()).unwrap_or("eq");
                let v = co.get("value").cloned().unwrap_or(Value::Null);

                let op_enum = match o_str {
                    "eq" => ComparisonOperator::Eq,
                    "gt" => ComparisonOperator::Gt,
                    "lt" => ComparisonOperator::Lt,
                    "like" | "contains" => ComparisonOperator::Contains,
                    _ => ComparisonOperator::Eq,
                };
                conditions.push(Condition {
                    field: f,
                    operator: op_enum,
                    value: v,
                });
            }
        }
    }
    Ok(QueryFilter {
        operator: op,
        conditions,
    })
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_parse_projection() {
        let p = parse_projection(&["name".into(), "age".into()]).unwrap();
        match p {
            Projection::Include(v) => assert_eq!(v.len(), 2),
            _ => panic!("Should be Include"),
        }

        let p_ex = parse_projection(&["-password".into()]).unwrap();
        match p_ex {
            Projection::Exclude(v) => assert_eq!(v[0], "password"),
            _ => panic!("Should be Exclude"),
        }
    }

    #[test]
    fn test_query_builder() {
        let q = QueryBuilder::new("users")
            .where_eq("active", json!(true))
            .select(vec!["username".into()])
            .unwrap()
            .build();

        assert_eq!(q.collection, "users");
        assert!(q.filter.is_some());
        assert!(q.projection.is_some());
    }

    #[test]
    fn test_parse_sort() {
        let s = parse_sort_specs(&["+age".into(), "name:desc".into()]).unwrap();
        assert_eq!(s[0].order, SortOrder::Asc);
        assert_eq!(s[1].order, SortOrder::Desc);
    }
}
