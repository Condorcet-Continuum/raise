// FICHIER : src-tauri/src/json_db/query/executor.rs

use crate::utils::json;
use crate::utils::prelude::*;
use crate::utils::{Future, Ordering, Pin};

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::indexes::manager::IndexManager;
use crate::json_db::query::{
    optimizer::QueryOptimizer, ComparisonOperator, Condition, FilterOperator, Projection, Query,
    QueryFilter, QueryResult, SortField, SortOrder,
};

// --- TRAIT ASYNC POUR L'INDEX ---
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

pub trait IndexProvider: Send + Sync {
    fn has_index<'a>(&'a self, collection: &'a str, field: &'a str) -> BoxFuture<'a, bool>;

    fn search<'a>(
        &'a self,
        collection: &'a str,
        field: &'a str,
        value: &'a Value,
    ) -> BoxFuture<'a, RaiseResult<Vec<String>>>;
}

// --- IMPLÉMENTATION NO-OP (BOUCHON) ---
pub struct NoOpIndexProvider;
impl IndexProvider for NoOpIndexProvider {
    fn has_index<'a>(&'a self, _c: &'a str, _f: &'a str) -> BoxFuture<'a, bool> {
        Box::pin(async { false })
    }
    fn search<'a>(
        &'a self,
        _c: &'a str,
        _f: &'a str,
        _v: &'a Value,
    ) -> BoxFuture<'a, RaiseResult<Vec<String>>> {
        Box::pin(async { Ok(vec![]) })
    }
}

// --- IMPLÉMENTATION RÉELLE (PONT VERS IndexManager) ---
pub struct RealIndexProvider<'a> {
    manager: IndexManager<'a>,
}

impl<'a> RealIndexProvider<'a> {
    pub fn new(manager: IndexManager<'a>) -> Self {
        Self { manager }
    }
}

impl<'a> IndexProvider for RealIndexProvider<'a> {
    fn has_index<'b>(&'b self, collection: &'b str, field: &'b str) -> BoxFuture<'b, bool> {
        Box::pin(async move { self.manager.has_index(collection, field).await })
    }

    fn search<'b>(
        &'b self,
        collection: &'b str,
        field: &'b str,
        value: &'b Value,
    ) -> BoxFuture<'b, RaiseResult<Vec<String>>> {
        Box::pin(async move { self.manager.search(collection, field, value).await })
    }
}

// --- MOTEUR DE REQUÊTE ---

pub struct QueryEngine<'a> {
    manager: &'a CollectionsManager<'a>,
    index_provider: Box<dyn IndexProvider + 'a>,
}

impl<'a> QueryEngine<'a> {
    pub fn new(manager: &'a CollectionsManager<'a>) -> Self {
        let idx_mgr = IndexManager::new(manager.storage, &manager.space, &manager.db);
        Self {
            manager,
            index_provider: Box::new(RealIndexProvider::new(idx_mgr)),
        }
    }

    /// Constructeur pour injection de dépendance (Tests)
    /// Modifie l'instance courante pour utiliser un provider spécifique
    pub fn with_index_provider(mut self, provider: Box<dyn IndexProvider + 'a>) -> Self {
        self.index_provider = provider;
        self
    }

    pub async fn execute_query(&self, mut query: Query) -> RaiseResult<QueryResult> {
        let optimizer = QueryOptimizer::new();
        query = optimizer.optimize(query)?;

        // 1. CHARGEMENT (Index vs Scan avec Résolution Fractale)
        let collection_paths = self.resolve_collection_path(&query.collection).await?;
        let mut documents = Vec::new();

        for actual_collection_path in collection_paths {
            let mut sub_query = query.clone();
            sub_query.collection = actual_collection_path.clone();

            let index_candidate = self.find_index_candidate(&sub_query).await;

            let mut batch_docs = match index_candidate {
                Some((field, value)) => {
                    let clean_val = self.strip_quotes(&value);
                    let clean_field = self.resolve_index_field(&field, &actual_collection_path);

                    #[cfg(debug_assertions)]
                    println!(
                        "⚡ QueryEngine: Index Hit sur {}.{}",
                        actual_collection_path, clean_field
                    );

                    let ids = self
                        .index_provider
                        .search(&actual_collection_path, &clean_field, &clean_val)
                        .await?;

                    self.manager
                        .read_many(&actual_collection_path, &ids)
                        .await?
                }
                None => {
                    #[cfg(debug_assertions)]
                    println!("🐢 QueryEngine: Full Scan sur {}", actual_collection_path);
                    self.manager.list_all(&actual_collection_path).await?
                }
            };
            documents.append(&mut batch_docs);
        }

        // 2. FILTRAGE
        if let Some(filter) = &query.filter {
            documents.retain(|doc| self.evaluate_filter(doc, filter, &query.collection));
        }

        // 3. TRI
        if let Some(sort_fields) = &query.sort {
            documents.sort_by(|a, b| self.compare_docs(a, b, sort_fields, &query.collection));
        }

        let total_count = documents.len() as u64;
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(documents.len());

        // 4. PAGINATION
        let mut paged_docs: Vec<Value> = documents.into_iter().skip(offset).take(limit).collect();

        // 5. PROJECTION
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

    async fn find_index_candidate(&self, query: &Query) -> Option<(String, Value)> {
        if let Some(filter) = &query.filter {
            if filter.operator == FilterOperator::And {
                for cond in &filter.conditions {
                    let clean_field = self.normalize_field_path(&cond.field, &query.collection);

                    if cond.operator == ComparisonOperator::Eq {
                        let has = self
                            .index_provider
                            .has_index(&query.collection, &clean_field)
                            .await;

                        if has {
                            return Some((cond.field.clone(), cond.value.clone()));
                        }
                    }

                    let leaf = cond.field.split('.').next_back().unwrap_or(&cond.field);
                    if leaf != clean_field && cond.operator == ComparisonOperator::Eq {
                        let has_leaf = self.index_provider.has_index(&query.collection, leaf).await;

                        if has_leaf {
                            return Some((cond.field.clone(), cond.value.clone()));
                        }
                    }
                }
            }
        }
        None
    }

    // --- LOGIQUE MÉTIER ---

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
            let cmp = self.compare_json_values(va, vb);
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
        self.compare_json_values(a, Some(b)).into()
    }

    fn compare_json_values(&self, a: Option<&Value>, b: Option<&Value>) -> Ordering {
        match (a, b) {
            (Some(v1), Some(v2)) => {
                if let (Some(n1), Some(n2)) = (v1.as_f64(), v2.as_f64()) {
                    return n1.partial_cmp(&n2).unwrap_or(Ordering::Equal);
                }
                if let (Some(s1), Some(s2)) = (v1.as_str(), v2.as_str()) {
                    return s1.cmp(s2);
                }
                if let (Some(b1), Some(b2)) = (v1.as_bool(), v2.as_bool()) {
                    return b1.cmp(&b2);
                }
                Ordering::Equal
            }
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
    }

    fn project_fields(&self, doc: &Value, projection: &Projection, collection_name: &str) -> Value {
        if let Value::Object(map) = doc {
            let mut new_map = json::Map::new();
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

    // Résolution intelligente des chemins de collection
    async fn resolve_collection_path(&self, target_collection: &str) -> RaiseResult<Vec<String>> {
        // Si le chemin contient déjà des '/', on assume qu'il est absolu
        if target_collection.contains('/') {
            return Ok(vec![target_collection.to_string()]);
        }

        let index_path = self
            .manager
            .storage
            .config
            .db_root(&self.manager.space, &self.manager.db)
            .join("_system.json");
        let mut resolved_paths = Vec::new();

        if let Ok(content) = tokio::fs::read_to_string(&index_path).await {
            if let Ok(index_json) = crate::utils::data::parse::<Value>(&content) {
                if let Some(collections) = index_json
                    .pointer("/collections")
                    .and_then(|v| v.as_object())
                {
                    for (path, _) in collections {
                        // On cherche si le chemin exact OU le dossier final correspond (ex: dapps/.../components)
                        if path == target_collection
                            || path.ends_with(&format!("/{}", target_collection))
                        {
                            resolved_paths.push(path.clone());
                        }
                    }
                }
            }
        }

        // Si non trouvé dans l'index, on fallback sur la racine classique
        if resolved_paths.is_empty() {
            resolved_paths.push(target_collection.to_string());
        }

        Ok(resolved_paths)
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use crate::utils::{io::tempdir, json::json, Arc, HashMap, Mutex};

    fn setup_test_db() -> (tempfile::TempDir, JsonDbConfig) {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        (dir, config)
    }

    // Mock Provider Async
    struct MockIndex {
        indexes: Arc<Mutex<HashMap<String, Vec<String>>>>,
    }

    impl IndexProvider for MockIndex {
        fn has_index<'a>(&'a self, _c: &'a str, field: &'a str) -> BoxFuture<'a, bool> {
            let idx = self.indexes.clone();
            let f = field.to_string();
            Box::pin(async move {
                let guard = idx.lock().unwrap();
                guard.contains_key(&f)
            })
        }
        fn search<'a>(
            &'a self,
            _c: &'a str,
            field: &'a str,
            _val: &'a Value,
        ) -> BoxFuture<'a, RaiseResult<Vec<String>>> {
            let idx = self.indexes.clone();
            let f = field.to_string();
            Box::pin(async move {
                let guard = idx.lock().unwrap();
                Ok(guard.get(&f).cloned().unwrap_or_default())
            })
        }
    }

    #[tokio::test]
    async fn test_full_query_execution() {
        let (_dir, config) = setup_test_db();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test", "db");
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
        manager.init_db().await.unwrap();

        let engine = QueryEngine::new(&manager);

        manager.create_collection("docs", None).await.unwrap();
        manager
            .insert_raw("docs", &json!({"id": "1", "tags": ["rust", "code"]}))
            .await
            .unwrap();

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

    #[tokio::test]
    async fn test_query_engine_uses_mock_index() {
        let (_dir, config) = setup_test_db();
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test", "db");
        manager.init_db().await.unwrap();
        manager.create_collection("users", None).await.unwrap();

        // On insère "user" et "admin"
        manager
            .insert_raw("users", &json!({"id": "1", "role": "admin"}))
            .await
            .unwrap();
        manager
            .insert_raw("users", &json!({"id": "2", "role": "user"}))
            .await
            .unwrap();

        // Le Mock index dit que "admin" correspond au document ID "1"
        let mut idx_map = HashMap::new();
        idx_map.insert("role".to_string(), vec!["1".to_string()]);
        let mock_provider = Box::new(MockIndex {
            indexes: Arc::new(Mutex::new(idx_map)),
        });

        // Injection du Mock via chaînage correct
        let engine = QueryEngine::new(&manager).with_index_provider(mock_provider);

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

        // Si l'index est utilisé, on charge seulement l'ID "1"
        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["id"], "1");
    }

    #[test]
    fn test_evaluate_condition_logic() {
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test", "db");
        let engine = QueryEngine::new(&manager);

        let doc = json!({ "age": 25, "tags": ["a", "b"] });

        // GT
        assert!(engine.evaluate_condition(&doc, &Condition::gt("age", json!(20)), "col"));
        // LT
        assert!(!engine.evaluate_condition(&doc, &Condition::lt("age", json!(20)), "col"));
        // CONTAINS
        assert!(engine.evaluate_condition(&doc, &Condition::contains("tags", json!("a")), "col"));
    }
}
