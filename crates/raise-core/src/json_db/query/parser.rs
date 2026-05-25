// FICHIER : src-tauri/src/json_db/query/parser.rs

use super::{
    ComparisonOperator, Condition, FilterOperator, Projection, Query, QueryFilter, SortField,
    SortOrder,
};

use crate::utils::prelude::*;

pub fn parse_projection(fields: &[String]) -> RaiseResult<Projection> {
    if fields.is_empty() {
        raise_error!(
            "ERR_DATA_FIELDS_EMPTY",
            error = "Opération impossible : la liste des champs est vide.",
            context = json_value!({
                "action": "validate_payload_integrity",
                "hint": "L'objet fourni ne contient aucune propriété exploitable. Vérifiez la source de données.",
                "severity": "medium"
            })
        );
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

    pub fn where_eq(mut self, field: impl Into<String>, value: JsonValue) -> Self {
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

pub fn parse_filter_from_json(value: &JsonValue) -> RaiseResult<QueryFilter> {
    // 1. Validation de la racine
    let Some(obj) = value.as_object() else {
        raise_error!(
            "ERR_QUERY_PARSE_TYPE",
            error = "Le filtre doit être un objet JSON.",
            context = json_value!({ "received": value })
        );
    };

    // 2. Extraction de l'opérateur logique (And/Or/Not)
    let op = match obj
        .get("operator")
        .and_then(|v| v.as_str())
        .unwrap_or("and")
        .to_lowercase()
        .as_str()
    {
        "or" => FilterOperator::Or,
        "not" => FilterOperator::Not,
        "and" => FilterOperator::And,
        _ => FilterOperator::And, // Ici on accepte 'and' par défaut
    };

    let mut conditions = Vec::new();

    // 3. Traitement strict des conditions
    if let Some(arr) = obj.get("conditions").and_then(|v| v.as_array()) {
        for (index, c) in arr.iter().enumerate() {
            let Some(co) = c.as_object() else {
                raise_error!(
                    "ERR_QUERY_PARSE_COND_TYPE",
                    error = format!("La condition #{} n'est pas un objet.", index)
                );
            };

            let field = match co
                .get("field")
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
            {
                Some(f) => f.to_string(),
                None => {
                    raise_error!(
                        "ERR_QUERY_PARSE_MISSING_FIELD",
                        error = format!(
                            "Le champ 'field' est manquant ou vide dans la condition #{}",
                            index
                        ),
                        context = json_value!({ "condition": co })
                    );
                }
            };

            let o_str = co
                .get("operator")
                .and_then(|v| v.as_str())
                .unwrap_or("eq")
                .to_lowercase();
            let op_enum = match o_str.as_str() {
                "eq" | "=" => ComparisonOperator::Eq,
                "ne" | "!=" | "<>" => ComparisonOperator::Ne,
                "gt" | ">" => ComparisonOperator::Gt,
                "gte" | ">=" => ComparisonOperator::Gte,
                "lt" | "<" => ComparisonOperator::Lt,
                "lte" | "<=" => ComparisonOperator::Lte,
                "in" => ComparisonOperator::In,
                "contains" => ComparisonOperator::Contains,
                "startswith" => ComparisonOperator::StartsWith,
                "endswith" => ComparisonOperator::EndsWith,
                "like" => ComparisonOperator::Like,
                "matches" => ComparisonOperator::Matches,
                "isa" | "is_a" => ComparisonOperator::IsA,
                "astrule" | "ast_rule" => ComparisonOperator::AstRule,
                _ => {
                    raise_error!(
                        "ERR_QUERY_PARSE_OPERATOR",
                        error =
                            format!("Opérateur '{}' inconnu dans la condition #{}", o_str, index),
                        context = json_value!({ "field": field, "operator": o_str })
                    );
                }
            };

            let value = co.get("value").cloned().unwrap_or(JsonValue::Null);

            conditions.push(Condition {
                field,
                operator: op_enum,
                value,
            });
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

    #[test]
    fn test_parse_filter_full() -> RaiseResult<()> {
        let json_input = json_value!({
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

        Ok(())
    }

    #[test]
    fn test_parse_projection() -> RaiseResult<()> {
        let p = parse_projection(&["name".into(), "age".into()]).unwrap();
        match p {
            Projection::Include(v) => assert_eq!(v.len(), 2),
            _ => {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "La projection devrait être de type 'Include'."
                );
            }
        }

        let p_ex = parse_projection(&["-password".into()]).unwrap();
        match p_ex {
            Projection::Exclude(v) => assert_eq!(v[0], "password"),
            _ => {
                raise_error!(
                    "ERR_TEST_ASSERTION_FAILED",
                    error = "La projection devrait être de type 'Exclude'."
                );
            }
        }

        Ok(())
    }

    #[test]
    fn test_query_builder() -> RaiseResult<()> {
        let q = QueryBuilder::new("users")
            .where_eq("active", json_value!(true))
            .limit(5)
            .sort("created_at", SortOrder::Desc)
            .select(vec!["handle".into()])?
            .build();

        assert_eq!(q.collection, "users");
        assert!(q.filter.is_some());
        assert_eq!(q.limit, Some(5));
        assert!(q.sort.is_some());
        assert!(q.projection.is_some());

        Ok(())
    }
}
