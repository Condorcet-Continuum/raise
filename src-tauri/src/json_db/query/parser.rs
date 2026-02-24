// FICHIER : src-tauri/src/json_db/query/parser.rs

use super::{
    ComparisonOperator, Condition, FilterOperator, Projection, Query, QueryFilter, SortField,
    SortOrder,
};

use crate::utils::prelude::*;

pub fn parse_projection(fields: &[String]) -> RaiseResult<Projection> {
    if fields.is_empty() {
        return Err(AppError::NotFound("message".to_string()));
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

    // Helper générique
    pub fn where_cond(mut self, condition: Condition) -> Self {
        self.add_cond(FilterOperator::And, condition);
        self
    }

    fn add_cond(&mut self, op: FilterOperator, c: Condition) {
        if let Some(ref mut f) = self.query.filter {
            if f.operator == op {
                f.conditions.push(c);
            } else {
                // Pour simplifier dans ce builder, on ajoute à la liste existante
                // Une implémentation plus complexe gérerait les groupes AND/OR imbriqués
                f.conditions.push(c);
            }
        } else {
            self.query.filter = Some(QueryFilter {
                operator: op,
                conditions: vec![c],
            });
        }
    }

    pub fn select(mut self, fields: Vec<String>) -> RaiseResult<Self> {
        self.query.projection = Some(parse_projection(&fields)?);
        Ok(self)
    }

    pub fn limit(mut self, limit: usize) -> Self {
        self.query.limit = Some(limit);
        self
    }

    pub fn offset(mut self, offset: usize) -> Self {
        self.query.offset = Some(offset);
        self
    }

    pub fn sort(mut self, field: &str, order: SortOrder) -> Self {
        let sort_field = SortField {
            field: field.into(),
            order,
        };
        if let Some(ref mut s) = self.query.sort {
            s.push(sort_field);
        } else {
            self.query.sort = Some(vec![sort_field]);
        }
        self
    }

    pub fn build(self) -> Query {
        self.query
    }
}

pub fn parse_sort_specs(specs: &[String]) -> RaiseResult<Vec<SortField>> {
    let mut out = Vec::new();
    for spec in specs {
        out.push(parse_single_sort_spec(spec)?);
    }
    Ok(out)
}

fn parse_single_sort_spec(spec: &str) -> RaiseResult<SortField> {
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

pub fn parse_filter_from_json(value: &Value) -> RaiseResult<QueryFilter> {
    let obj = value
        .as_object()
        .ok_or_else(|| AppError::Validation("Not an object".to_string()))?;

    let op = match obj
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("and")
        .to_lowercase()
        .as_str()
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

                let o_str = co
                    .get("operator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("eq")
                    .to_lowercase();

                let v = co.get("value").cloned().unwrap_or(Value::Null);

                let op_enum = match o_str.as_str() {
                    "eq" | "=" => ComparisonOperator::Eq,
                    "ne" | "!=" | "<>" => ComparisonOperator::Ne,
                    "gt" | ">" => ComparisonOperator::Gt,
                    "gte" | ">=" => ComparisonOperator::Gte,
                    "lt" | "<" => ComparisonOperator::Lt,
                    "lte" | "<=" => ComparisonOperator::Lte,
                    "in" => ComparisonOperator::In,
                    "contains" | "contain" => ComparisonOperator::Contains,
                    "startswith" | "starts_with" => ComparisonOperator::StartsWith,
                    "endswith" | "ends_with" => ComparisonOperator::EndsWith,
                    "like" => ComparisonOperator::Like,
                    "matches" | "regex" => ComparisonOperator::Matches,
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
    use crate::utils::json::json;

    #[test]
    fn test_parse_filter_full() {
        let json_input = json!({
            "operator": "and",
            "conditions": [
                {"field": "age", "operator": "gte", "value": 18},
                {"field": "name", "operator": "startswith", "value": "A"}
            ]
        });

        let filter = parse_filter_from_json(&json_input).unwrap();
        assert_eq!(filter.operator, FilterOperator::And);
        assert_eq!(filter.conditions.len(), 2);
        assert_eq!(filter.conditions[0].operator, ComparisonOperator::Gte);
        assert_eq!(
            filter.conditions[1].operator,
            ComparisonOperator::StartsWith
        );
    }

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
            .limit(5)
            .sort("created_at", SortOrder::Desc)
            .select(vec!["username".into()])
            .unwrap()
            .build();

        assert_eq!(q.collection, "users");
        assert!(q.filter.is_some());
        assert_eq!(q.limit, Some(5));
        assert!(q.sort.is_some());
        assert!(q.projection.is_some());
    }
}
