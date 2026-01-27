// FICHIER : src-tauri/src/json_db/query/executor.rs

use anyhow::Result;
use serde_json::Value;
use std::cmp::Ordering;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::query::{
    optimizer::QueryOptimizer, ComparisonOperator, Condition, FilterOperator, Projection, Query,
    QueryFilter, QueryResult, SortField, SortOrder,
};

pub trait IndexProvider: Send + Sync {
    fn has_index(&self, collection: &str, field: &str) -> bool;
    fn search(&self, collection: &str, field: &str, value: &Value) -> Result<Vec<String>>;
}

pub struct NoOpIndexProvider;
impl IndexProvider for NoOpIndexProvider {
    fn has_index(&self, _c: &str, _f: &str) -> bool {
        false
    }
    fn search(&self, _c: &str, _f: &str, _v: &Value) -> Result<Vec<String>> {
        Ok(vec![])
    }
}

pub struct QueryEngine<'a> {
    manager: &'a CollectionsManager<'a>,
    index_provider: Box<dyn IndexProvider>,
}

impl<'a> QueryEngine<'a> {
    pub fn new(manager: &'a CollectionsManager<'a>) -> Self {
        Self {
            manager,
            index_provider: Box::new(NoOpIndexProvider),
        }
    }

    pub fn with_index_provider(mut self, provider: Box<dyn IndexProvider>) -> Self {
        self.index_provider = provider;
        self
    }

    pub async fn execute_query(&self, mut query: Query) -> Result<QueryResult> {
        let optimizer = QueryOptimizer::new();
        query = optimizer.optimize(query)?;

        // --- CHARGEMENT (Modifié en Async) ---
        let mut documents = match self.find_index_candidate(&query) {
            Some((field, value)) => {
                let clean_val = self.strip_quotes(&value);
                let clean_field = self.resolve_index_field(&field, &query.collection);

                #[cfg(debug_assertions)]
                println!(
                    "⚡ QueryEngine: Index Hit sur {}.{}",
                    query.collection, clean_field
                );

                let ids =
                    self.index_provider
                        .search(&query.collection, &clean_field, &clean_val)?;
                // Appel async au manager
                self.manager.read_many(&query.collection, &ids).await?
            }
            None => {
                #[cfg(debug_assertions)]
                println!("🐢 QueryEngine: Full Scan sur {}", query.collection);
                // Appel async au manager
                self.manager.list_all(&query.collection).await?
            }
        };

        // --- FILTRAGE ---
        if let Some(filter) = &query.filter {
            documents.retain(|doc| self.evaluate_filter(doc, filter, &query.collection));
        }

        // --- TRI ---
        if let Some(sort_fields) = &query.sort {
            documents.sort_by(|a, b| self.compare_docs(a, b, sort_fields, &query.collection));
        }

        let total_count = documents.len() as u64;
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(documents.len());

        // --- PAGINATION ---
        let mut paged_docs: Vec<Value> = documents.into_iter().skip(offset).take(limit).collect();

        // --- PROJECTION ---
        if let Some(projection) = &query.projection {
            for doc in &mut paged_docs {
                *doc = self.project_fields(doc, projection, &query.collection);
            }
        }

        Ok(QueryResult {
            documents: paged_docs,
            total_count,
            offset: Some(offset),
            limit: Some(limit),
        })
    }

    fn find_index_candidate(&self, query: &Query) -> Option<(String, Value)> {
        if let Some(filter) = &query.filter {
            if filter.operator == FilterOperator::And {
                for cond in &filter.conditions {
                    let clean_field = self.normalize_field_path(&cond.field, &query.collection);
                    if cond.operator == ComparisonOperator::Eq
                        && self
                            .index_provider
                            .has_index(&query.collection, &clean_field)
                    {
                        return Some((cond.field.clone(), cond.value.clone()));
                    }
                    let leaf = cond.field.split('.').next_back().unwrap_or(&cond.field);
                    if leaf != clean_field
                        && cond.operator == ComparisonOperator::Eq
                        && self.index_provider.has_index(&query.collection, leaf)
                    {
                        return Some((cond.field.clone(), cond.value.clone()));
                    }
                }
            }
        }
        None
    }

    fn resolve_index_field(&self, raw_field: &str, collection: &str) -> String {
        let norm = self.normalize_field_path(raw_field, collection);
        if norm.contains('.') {
            return norm.split('.').next_back().unwrap_or(&norm).to_string();
        }
        norm
    }

    fn evaluate_filter(
        &self,
        document: &Value,
        filter: &QueryFilter,
        collection_name: &str,
    ) -> bool {
        match filter.operator {
            FilterOperator::And => filter
                .conditions
                .iter()
                .all(|c| self.evaluate_condition(document, c, collection_name)),
            FilterOperator::Or => filter
                .conditions
                .iter()
                .any(|c| self.evaluate_condition(document, c, collection_name)),
            FilterOperator::Not => !filter
                .conditions
                .iter()
                .any(|c| self.evaluate_condition(document, c, collection_name)),
        }
    }

    fn evaluate_condition(
        &self,
        document: &Value,
        condition: &Condition,
        collection_name: &str,
    ) -> bool {
        let val = self.get_field_value_smart(document, &condition.field, collection_name);
        let clean_cond_val = self.strip_quotes(&condition.value);

        match &condition.operator {
            ComparisonOperator::Eq => self.values_equal(val, Some(&clean_cond_val)),
            ComparisonOperator::Ne => !self.values_equal(val, Some(&clean_cond_val)),
            ComparisonOperator::Matches => self.values_equal(val, Some(&clean_cond_val)),

            ComparisonOperator::Gt => {
                self.compare_values(val, &clean_cond_val) == Some(Ordering::Greater)
            }
            ComparisonOperator::Gte => {
                let o = self.compare_values(val, &clean_cond_val);
                o == Some(Ordering::Greater) || o == Some(Ordering::Equal)
            }
            ComparisonOperator::Lt => {
                self.compare_values(val, &clean_cond_val) == Some(Ordering::Less)
            }
            ComparisonOperator::Lte => {
                let o = self.compare_values(val, &clean_cond_val);
                o == Some(Ordering::Less) || o == Some(Ordering::Equal)
            }

            ComparisonOperator::Contains => match (val, &clean_cond_val) {
                (Some(Value::String(s)), Value::String(sub)) => {
                    s.to_lowercase().contains(&sub.to_lowercase())
                }
                (Some(Value::Array(arr)), v) => {
                    if arr.contains(v) {
                        return true;
                    }

                    if let Value::Array(sub_arr) = v {
                        if sub_arr.iter().all(|sub_item| arr.contains(sub_item)) {
                            return true;
                        }
                    }

                    let v_str = match v {
                        Value::String(s) => s.to_lowercase(),
                        _ => v.to_string().to_lowercase(),
                    };
                    arr.iter().any(|item| {
                        let item_str = match item {
                            Value::String(s) => s.to_lowercase(),
                            _ => item.to_string().to_lowercase(),
                        };
                        item_str == v_str
                    })
                }
                _ => false,
            },

            ComparisonOperator::StartsWith => match (val, &clean_cond_val) {
                (Some(Value::String(s)), Value::String(prefix)) => s.starts_with(prefix),
                _ => false,
            },
            ComparisonOperator::EndsWith => match (val, &clean_cond_val) {
                (Some(Value::String(s)), Value::String(suffix)) => s.ends_with(suffix),
                _ => false,
            },
            ComparisonOperator::In => {
                if let Some(doc_val) = val {
                    if let Some(target_list) = clean_cond_val.as_array() {
                        return target_list.contains(doc_val);
                    }
                }
                false
            }

            ComparisonOperator::Like => {
                if let Some(Value::String(s)) = val {
                    return self.match_like_smart(s, &clean_cond_val);
                }
                if let Some(Value::Array(arr)) = val {
                    return arr.iter().any(|item| {
                        if let Value::String(s) = item {
                            self.match_like_smart(s, &clean_cond_val)
                        } else {
                            self.match_like_smart(&item.to_string(), &clean_cond_val)
                        }
                    });
                }
                false
            }
        }
    }

    fn match_like_smart(&self, text: &str, pattern_val: &Value) -> bool {
        let pattern_str = match pattern_val {
            Value::String(s) => s,
            _ => return false,
        };

        let text_lower = text.to_lowercase();
        let pat_lower = pattern_str.to_lowercase();

        if !pat_lower.contains('%') {
            return text_lower.contains(&pat_lower);
        }

        let parts: Vec<&str> = pat_lower.split('%').collect();
        let mut current_pos = 0;

        if let Some(first) = parts.first() {
            if !first.is_empty() {
                if !text_lower.starts_with(first) {
                    return false;
                }
                current_pos += first.len();
            }
        }

        let len_parts = parts.len();
        if len_parts > 2 {
            for &part in &parts[1..len_parts - 1] {
                if part.is_empty() {
                    continue;
                }
                match text_lower[current_pos..].find(part) {
                    Some(offset) => current_pos += offset + part.len(),
                    None => return false,
                }
            }
        }

        if let Some(&last) = parts.last() {
            if !last.is_empty() && !text_lower.ends_with(last) {
                return false;
            }
        }
        true
    }

    fn get_field_value_smart<'b>(
        &self,
        doc: &'b Value,
        raw_path: &str,
        collection_name: &str,
    ) -> Option<&'b Value> {
        let clean_raw = self.clean_field_name_quotes(raw_path);
        let norm_path = self.normalize_field_path(&clean_raw, collection_name);

        if let Some(v) = self.get_field_value_deep_case_insensitive(doc, &norm_path) {
            return Some(v);
        }

        if let Some(leaf) = clean_raw.split('.').next_back() {
            if leaf != norm_path {
                if let Some(v) = self.get_field_value_deep_case_insensitive(doc, leaf) {
                    return Some(v);
                }
            }
        }

        if let Value::Object(map) = doc {
            for val in map.values() {
                if val.is_object() {
                    if let Some(v) = self.get_field_value_deep_case_insensitive(val, &norm_path) {
                        return Some(v);
                    }
                    if let Some(leaf) = clean_raw.split('.').next_back() {
                        if let Some(v) = self.get_field_value_deep_case_insensitive(val, leaf) {
                            return Some(v);
                        }
                    }
                }
            }
        }
        None
    }

    fn get_field_value_deep_case_insensitive<'b>(
        &self,
        doc: &'b Value,
        path: &str,
    ) -> Option<&'b Value> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = doc;
        for part in parts {
            match current {
                Value::Object(map) => {
                    if let Some(v) = map.get(part) {
                        current = v;
                    } else if let Some((_, v)) =
                        map.iter().find(|(k, _)| k.eq_ignore_ascii_case(part))
                    {
                        current = v;
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(current)
    }

    fn normalize_field_path(&self, raw_path: &str, collection_name: &str) -> String {
        let mut field = self.clean_field_name_quotes(raw_path);
        let prefix = format!("{}.", collection_name);
        if field.len() > prefix.len() && field[..prefix.len()].eq_ignore_ascii_case(&prefix) {
            field = field[prefix.len()..].to_string();
        }
        field
    }

    fn clean_field_name_quotes(&self, field: &str) -> String {
        let trimmed = field.trim();
        let len = trimmed.len();
        if len >= 2
            && ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
                || (trimmed.starts_with('"') && trimmed.ends_with('"')))
        {
            return trimmed[1..len - 1].to_string();
        }
        trimmed.to_string()
    }

    fn strip_quotes(&self, val: &Value) -> Value {
        if let Value::String(s) = val {
            let mut processing = s.clone();
            loop {
                let trimmed = processing.trim();
                let len = trimmed.len();
                if len >= 2
                    && ((trimmed.starts_with('\'') && trimmed.ends_with('\''))
                        || (trimmed.starts_with('"') && trimmed.ends_with('"')))
                {
                    processing = trimmed[1..len - 1].to_string();
                } else {
                    break;
                }
            }
            if processing != *s {
                return Value::String(processing);
            }
        }
        val.clone()
    }

    fn values_equal(&self, a: Option<&Value>, b: Option<&Value>) -> bool {
        match (a, b) {
            (Some(v1), Some(v2)) => {
                if v1 == v2 {
                    return true;
                }
                if let (Some(n1), Some(n2)) = (v1.as_f64(), v2.as_f64()) {
                    return (n1 - n2).abs() < f64::EPSILON;
                }
                false
            }
            (None, None) => true,
            _ => false,
        }
    }

    fn compare_docs(
        &self,
        a: &Value,
        b: &Value,
        sort_fields: &[SortField],
        collection_name: &str,
    ) -> Ordering {
        for s in sort_fields {
            let va = self.get_field_value_smart(a, &s.field, collection_name);
            let vb = self.get_field_value_smart(b, &s.field, collection_name);
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

    fn project_fields(&self, doc: &Value, projection: &Projection, collection_name: &str) -> Value {
        if let Value::Object(map) = doc {
            let mut new_map = serde_json::Map::new();
            match projection {
                Projection::Include(fields) => {
                    if fields.is_empty() {
                        return doc.clone();
                    }
                    for field in fields {
                        if let Some(val) = self.get_field_value_smart(doc, field, collection_name) {
                            let output_key = field.split('.').next_back().unwrap_or(field);
                            new_map.insert(output_key.to_string(), val.clone());
                        }
                    }
                }
                Projection::Exclude(fields) => {
                    for (k, v) in map {
                        let banned = fields.iter().any(|f| {
                            let clean_f = self.normalize_field_path(f, collection_name);
                            clean_f.eq_ignore_ascii_case(k)
                                || f.split('.')
                                    .next_back()
                                    .unwrap_or(f)
                                    .eq_ignore_ascii_case(k)
                        });
                        if !banned {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::tempdir;

    fn setup_test_db() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    #[allow(dead_code)]
    struct MockIndex {
        indexes: HashMap<String, Vec<String>>,
    }

    impl IndexProvider for MockIndex {
        fn has_index(&self, _c: &str, field: &str) -> bool {
            self.indexes.contains_key(field)
        }
        fn search(&self, _c: &str, field: &str, _val: &Value) -> Result<Vec<String>> {
            Ok(self.indexes.get(field).cloned().unwrap_or_default())
        }
    }

    #[tokio::test]
    async fn test_full_query_execution() {
        let (_dir, config) = setup_test_db();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test", "db");
        // Setup async
        manager.init_db().await.unwrap();

        let engine = QueryEngine::new(&manager);

        manager.create_collection("users", None).await.unwrap();
        manager
            .insert_raw("users", &json!({"id": "1", "age": 20, "role": "user"}))
            .await
            .unwrap();
        manager
            .insert_raw("users", &json!({"id": "2", "age": 30, "role": "admin"}))
            .await
            .unwrap();

        let query = Query {
            collection: "users".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("role", json!("admin"))],
            }),
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        };

        let result = engine.execute_query(query).await.unwrap();
        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["id"], "2");
    }

    #[tokio::test]
    async fn test_smart_like_and_array() {
        let (_dir, config) = setup_test_db();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test", "db");
        // Setup async
        manager.init_db().await.unwrap();

        let engine = QueryEngine::new(&manager);

        manager.create_collection("docs", None).await.unwrap();
        manager
            .insert_raw("docs", &json!({"id": "1", "tags": ["rust", "code"]}))
            .await
            .unwrap();

        // Test ARRAY LIKE "rust"
        let query = Query {
            collection: "docs".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::new(
                    "tags",
                    ComparisonOperator::Like,
                    json!("rust"),
                )],
            }),
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        };

        let result = engine.execute_query(query).await.unwrap();
        assert_eq!(result.total_count, 1);
    }
}
