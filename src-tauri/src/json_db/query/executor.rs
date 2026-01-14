// FICHIER : src-tauri/src/json_db/query/executor.rs

use anyhow::Result;
use serde_json::Value;
use std::cmp::Ordering;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{
    optimizer::QueryOptimizer, ComparisonOperator, Condition, FilterOperator, Projection, Query,
    QueryFilter, QueryResult, SortField, SortOrder,
};

pub struct QueryEngine<'a> {
    manager: &'a CollectionsManager<'a>,
}

impl<'a> QueryEngine<'a> {
    pub fn new(manager: &'a CollectionsManager<'a>) -> Self {
        Self { manager }
    }

    pub async fn execute_query(&self, mut query: Query) -> Result<QueryResult> {
        let optimizer = QueryOptimizer::new();
        // On remplace la requête brute par sa version optimisée
        query = optimizer.optimize(query)?;

        // 1. Chargement (Optimisable plus tard avec des streams/iterateurs)
        let mut documents = self.manager.list_all(&query.collection)?;

        // 2. Filtrage
        if let Some(filter) = &query.filter {
            documents.retain(|doc| self.evaluate_filter(doc, filter));
        }

        // 3. Tri
        if let Some(sort_fields) = &query.sort {
            documents.sort_by(|a, b| self.compare_docs(a, b, sort_fields));
        }

        let total_count = documents.len() as u64;
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(documents.len());

        // 4. Pagination
        let mut paged_docs: Vec<Value> = documents.into_iter().skip(offset).take(limit).collect();

        // 5. PROJECTION (Selection des champs)
        if let Some(projection) = &query.projection {
            for doc in &mut paged_docs {
                *doc = self.project_fields(doc, projection);
            }
        }

        Ok(QueryResult {
            documents: paged_docs,
            total_count,
            offset: Some(offset),
            limit: Some(limit),
        })
    }

    fn project_fields(&self, doc: &Value, projection: &Projection) -> Value {
        if let Value::Object(map) = doc {
            let mut new_map = serde_json::Map::new();
            match projection {
                Projection::Include(fields) => {
                    if fields.is_empty() {
                        return doc.clone();
                    }
                    for field in fields {
                        if let Some(val) = map.get(field) {
                            new_map.insert(field.clone(), val.clone());
                        }
                    }
                }
                Projection::Exclude(fields) => {
                    for (k, v) in map {
                        if !fields.contains(k) {
                            new_map.insert(k.clone(), v.clone());
                        }
                    }
                }
            }
            Value::Object(new_map)
        } else {
            doc.clone()
        }
    }

    fn evaluate_filter(&self, document: &Value, filter: &QueryFilter) -> bool {
        match filter.operator {
            FilterOperator::And => filter
                .conditions
                .iter()
                .all(|c| self.evaluate_condition(document, c)),
            FilterOperator::Or => filter
                .conditions
                .iter()
                .any(|c| self.evaluate_condition(document, c)),
            FilterOperator::Not => !filter
                .conditions
                .iter()
                .any(|c| self.evaluate_condition(document, c)),
        }
    }

    fn evaluate_condition(&self, document: &Value, condition: &Condition) -> bool {
        let val = self.get_field_value(document, &condition.field);
        match &condition.operator {
            ComparisonOperator::Eq => val == Some(&condition.value),
            ComparisonOperator::Ne => val != Some(&condition.value),
            ComparisonOperator::Gt => {
                self.compare_values(val, &condition.value) == Some(Ordering::Greater)
            }
            ComparisonOperator::Gte => {
                let o = self.compare_values(val, &condition.value);
                o == Some(Ordering::Greater) || o == Some(Ordering::Equal)
            }
            ComparisonOperator::Lt => {
                self.compare_values(val, &condition.value) == Some(Ordering::Less)
            }
            ComparisonOperator::Lte => {
                let o = self.compare_values(val, &condition.value);
                o == Some(Ordering::Less) || o == Some(Ordering::Equal)
            }
            ComparisonOperator::Contains | ComparisonOperator::Like => {
                if let (Some(s1), Some(s2)) =
                    (val.and_then(|v| v.as_str()), condition.value.as_str())
                {
                    s1.contains(s2)
                } else if let (Some(arr), _) = (val.and_then(|v| v.as_array()), &condition.value) {
                    arr.contains(&condition.value)
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    fn compare_docs(&self, a: &Value, b: &Value, sort_fields: &[SortField]) -> Ordering {
        for s in sort_fields {
            let va = self.get_field_value(a, &s.field);
            let vb = self.get_field_value(b, &s.field);
            let cmp = match (va, vb) {
                (Some(x), Some(y)) => self.compare_json_values(x, y),
                (None, Some(_)) => Ordering::Less,
                (Some(_), None) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            };
            if cmp != Ordering::Equal {
                return match s.order {
                    SortOrder::Asc => cmp,
                    SortOrder::Desc => cmp.reverse(),
                };
            }
        }
        Ordering::Equal
    }

    fn get_field_value<'b>(&self, doc: &'b Value, path: &str) -> Option<&'b Value> {
        if !path.contains('.') {
            return doc.get(path);
        }
        doc.pointer(&format!("/{}", path.replace('.', "/")))
    }

    fn compare_values(&self, a: Option<&Value>, b: &Value) -> Option<Ordering> {
        a.map(|v| self.compare_json_values(v, b))
    }

    fn compare_json_values(&self, a: &Value, b: &Value) -> Ordering {
        if let (Some(n1), Some(n2)) = (a.as_f64(), b.as_f64()) {
            return n1.partial_cmp(&n2).unwrap_or(Ordering::Equal);
        }
        if let (Some(s1), Some(s2)) = (a.as_str(), b.as_str()) {
            return s1.cmp(s2);
        }
        if let (Some(b1), Some(b2)) = (a.as_bool(), b.as_bool()) {
            return b1.cmp(&b2);
        }
        a.to_string().cmp(&b.to_string())
    }
}

// ============================================================================
// TESTS D'INTÉGRATION
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_full_query_execution() {
        // 1. Setup DB
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test", "db");
        let engine = QueryEngine::new(&manager);

        // 2. Insert Data
        manager.create_collection("users", None).unwrap();
        manager
            .insert_raw("users", &json!({"id": "1", "age": 20, "role": "user"}))
            .unwrap();
        manager
            .insert_raw("users", &json!({"id": "2", "age": 30, "role": "admin"}))
            .unwrap();
        manager
            .insert_raw("users", &json!({"id": "3", "age": 40, "role": "user"}))
            .unwrap();

        // 3. Build Query: SELECT id FROM users WHERE age > 25 ORDER BY age DESC
        let query = Query {
            collection: "users".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("role", json!("user"))],
            }),
            sort: Some(vec![SortField {
                field: "age".into(),
                order: SortOrder::Desc,
            }]),
            limit: None,
            offset: None,
            projection: Some(Projection::Include(vec!["id".into()])),
        };

        // 4. Execute
        let result = engine.execute_query(query).await.unwrap();

        // 5. Verify
        assert_eq!(result.total_count, 2); // id 1 and 3 match "user"
        assert_eq!(result.documents[0]["id"], "3"); // Age 40 (Desc)
        assert_eq!(result.documents[1]["id"], "1"); // Age 20
        assert!(result.documents[0].get("age").is_none()); // Projection exclut age
    }
}
