// FICHIER : src-tauri/src/json_db/query/executor.rs

use crate::utils::prelude::*;

use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::indexes::manager::IndexManager;
use crate::json_db::query::{
    optimizer::QueryOptimizer, ComparisonOperator, Condition, FilterOperator, Projection, Query,
    QueryFilter, QueryResult, SortField, SortOrder,
};

// --- TRAIT ASYNC POUR L'INDEX ---
pub type BoxFuture<'a, T> = Pinned<Box<dyn AsyncFuture<Output = T> + Send + 'a>>;

pub trait IndexProvider: Send + Sync {
    fn has_index<'a>(&'a self, collection: &'a str, field: &'a str) -> BoxFuture<'a, bool>;

    fn search<'a>(
        &'a self,
        collection: &'a str,
        field: &'a str,
        value: &'a JsonValue,
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
        _v: &'a JsonValue,
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
        value: &'b JsonValue,
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

        let collection_paths = self.resolve_collection_path(&query.collection).await?;
        let mut documents = Vec::new();

        // 🎯 INTERCEPTION DE LA CLÉ PRIMAIRE (O(1))
        // Si la requête cherche un "_id" ou un "@id", on ne sollicite pas le moteur d'index secondaire.
        let mut primary_key_val = None;
        if let Some(filter) = &query.filter {
            for cond in &filter.conditions {
                if cond.operator == ComparisonOperator::Eq
                    && (cond.field == "_id" || cond.field == "@id")
                {
                    if let Some(s) = cond.value.as_str() {
                        primary_key_val = Some(s.to_string());
                        break;
                    }
                }
            }
        }

        for actual_collection_path in collection_paths {
            // 🚀 CAS 1 : C'est une recherche par Clé Primaire ! Temps d'accès : O(1)
            if let Some(ref pk) = primary_key_val {
                #[cfg(debug_assertions)]
                println!(
                    "⚡ QueryEngine: Primary Key Hit sur {} -> {}",
                    actual_collection_path, pk
                );

                if let Ok(Some(doc)) = self.manager.get_document(&actual_collection_path, pk).await
                {
                    documents.push(doc);
                }
                continue; // On passe à la collection suivante sans scanner
            }

            // 🔍 CAS 2 : C'est une requête complexe, on utilise les index secondaires
            let mut sub_query = query.clone();
            sub_query.collection = actual_collection_path.clone();

            let index_candidate = self.find_index_candidate(&sub_query).await;

            let mut batch_docs = match index_candidate {
                Some((_field, value, index_field_name)) => {
                    let clean_val = self.strip_quotes(&value);

                    // La vérification `has_index` a eu lieu, on peut chercher en sécurité.
                    match self
                        .index_provider
                        .search(&actual_collection_path, &index_field_name, &clean_val)
                        .await
                    {
                        Ok(ids) => {
                            #[cfg(debug_assertions)]
                            println!(
                                "⚡ QueryEngine: Index Hit sur {}.{}",
                                actual_collection_path, index_field_name
                            );

                            self.manager
                                .read_many(&actual_collection_path, &ids)
                                .await?
                        }
                        Err(_) => {
                            // Repli silencieux
                            self.manager.list_all(&actual_collection_path).await?
                        }
                    }
                }
                None => {
                    // Repli silencieux
                    self.manager.list_all(&actual_collection_path).await?
                }
            };
            documents.append(&mut batch_docs);
        }

        // 2. FILTRAGE (Post-chargement)
        if let Some(filter) = &query.filter {
            documents.retain(|doc| self.evaluate_filter(doc, filter, &query.collection));
        }

        // 3. TRI, PAGINATION, PROJECTION
        if let Some(sort_fields) = &query.sort {
            documents.sort_by(|a, b| self.compare_docs(a, b, sort_fields, &query.collection));
        }

        let total_count = documents.len() as u64;
        let offset = query.offset.unwrap_or(0);
        let limit = query.limit.unwrap_or(documents.len());

        let mut paged_docs: Vec<JsonValue> =
            documents.into_iter().skip(offset).take(limit).collect();

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

    /// 🎯 RECHERCHE D'INDEX ROBUSTE
    /// Retourne : (Nom du champ dans le document, Valeur cherchée, Nom de l'index à utiliser)
    async fn find_index_candidate(&self, query: &Query) -> Option<(String, JsonValue, String)> {
        let filter = query.filter.as_ref()?;

        for cond in &filter.conditions {
            if cond.operator != ComparisonOperator::Eq {
                continue;
            }

            let clean_field = self.normalize_field_path(&cond.field, &query.collection);
            let leaf = cond.field.split('.').next_back().unwrap_or(&cond.field);

            // On VÉRIFIE systématiquement l'existence de l'index avant de valider le candidat
            if self
                .index_provider
                .has_index(&query.collection, &clean_field)
                .await
            {
                return Some((cond.field.clone(), cond.value.clone(), clean_field));
            }

            if leaf != clean_field && self.index_provider.has_index(&query.collection, leaf).await {
                return Some((cond.field.clone(), cond.value.clone(), leaf.to_string()));
            }
        }

        None
    }

    // --- LOGIQUE MÉTIER ET NORMALISATION ---

    fn evaluate_filter(
        &self,
        document: &JsonValue,
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
        document: &JsonValue,
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
                self.compare_values(val, &clean_cond_val) == Some(FmtOrdering::Greater)
            }
            ComparisonOperator::Gte => {
                let o = self.compare_values(val, &clean_cond_val);
                o == Some(FmtOrdering::Greater) || o == Some(FmtOrdering::Equal)
            }
            ComparisonOperator::Lt => {
                self.compare_values(val, &clean_cond_val) == Some(FmtOrdering::Less)
            }
            ComparisonOperator::Lte => {
                let o = self.compare_values(val, &clean_cond_val);
                o == Some(FmtOrdering::Less) || o == Some(FmtOrdering::Equal)
            }

            ComparisonOperator::Contains => match (val, &clean_cond_val) {
                (Some(JsonValue::String(s)), JsonValue::String(sub)) => {
                    s.to_lowercase().contains(&sub.to_lowercase())
                }
                (Some(JsonValue::Array(arr)), v) => {
                    if arr.contains(v) {
                        return true;
                    }
                    let v_str = match v {
                        JsonValue::String(s) => s.to_lowercase(),
                        _ => v.to_string().to_lowercase(),
                    };
                    arr.iter().any(|item| {
                        let item_str = match item {
                            JsonValue::String(s) => s.to_lowercase(),
                            _ => item.to_string().to_lowercase(),
                        };
                        item_str == v_str
                    })
                }
                _ => false,
            },

            ComparisonOperator::StartsWith => match (val, &clean_cond_val) {
                (Some(JsonValue::String(s)), JsonValue::String(prefix)) => s.starts_with(prefix),
                _ => false,
            },
            ComparisonOperator::EndsWith => match (val, &clean_cond_val) {
                (Some(JsonValue::String(s)), JsonValue::String(suffix)) => s.ends_with(suffix),
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
                if let Some(JsonValue::String(s)) = val {
                    return self.match_like_smart(s, &clean_cond_val);
                }
                if let Some(JsonValue::Array(arr)) = val {
                    return arr.iter().any(|item| {
                        if let JsonValue::String(s) = item {
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

    fn match_like_smart(&self, text: &str, pattern_val: &JsonValue) -> bool {
        let pattern_str = match pattern_val {
            JsonValue::String(s) => s,
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
        doc: &'b JsonValue,
        raw_path: &str,
        collection_name: &str,
    ) -> Option<&'b JsonValue> {
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

        if let JsonValue::Object(map) = doc {
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
        doc: &'b JsonValue,
        path: &str,
    ) -> Option<&'b JsonValue> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = doc;
        for part in parts {
            match current {
                JsonValue::Object(map) => {
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

    fn strip_quotes(&self, val: &JsonValue) -> JsonValue {
        if let JsonValue::String(s) = val {
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
                return JsonValue::String(processing);
            }
        }
        val.clone()
    }

    fn values_equal(&self, a: Option<&JsonValue>, b: Option<&JsonValue>) -> bool {
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
        a: &JsonValue,
        b: &JsonValue,
        sort_fields: &[SortField],
        collection_name: &str,
    ) -> FmtOrdering {
        for s in sort_fields {
            let va = self.get_field_value_smart(a, &s.field, collection_name);
            let vb = self.get_field_value_smart(b, &s.field, collection_name);
            let cmp = self.compare_json_values(va, vb);
            if cmp != FmtOrdering::Equal {
                return match s.order {
                    SortOrder::Asc => cmp,
                    SortOrder::Desc => cmp.reverse(),
                };
            }
        }
        FmtOrdering::Equal
    }

    fn compare_values(&self, a: Option<&JsonValue>, b: &JsonValue) -> Option<FmtOrdering> {
        self.compare_json_values(a, Some(b)).into()
    }

    fn compare_json_values(&self, a: Option<&JsonValue>, b: Option<&JsonValue>) -> FmtOrdering {
        match (a, b) {
            (Some(v1), Some(v2)) => {
                if let (Some(n1), Some(n2)) = (v1.as_f64(), v2.as_f64()) {
                    return n1.partial_cmp(&n2).unwrap_or(FmtOrdering::Equal);
                }
                if let (Some(s1), Some(s2)) = (v1.as_str(), v2.as_str()) {
                    return s1.cmp(s2);
                }
                if let (Some(b1), Some(b2)) = (v1.as_bool(), v2.as_bool()) {
                    return b1.cmp(&b2);
                }
                FmtOrdering::Equal
            }
            (None, Some(_)) => FmtOrdering::Less,
            (Some(_), None) => FmtOrdering::Greater,
            (None, None) => FmtOrdering::Equal,
        }
    }

    fn project_fields(
        &self,
        doc: &JsonValue,
        projection: &Projection,
        collection_name: &str,
    ) -> JsonValue {
        if let JsonValue::Object(map) = doc {
            let mut new_map = JsonObject::new();
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
            JsonValue::Object(new_map)
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

        let mut resolved_paths = Vec::new();

        // 🎯 RETOUR EN ARRIÈRE VITAL : On bypass le Manager pour éviter une boucle infinie avec l'ACL !
        // Le moteur de requête DOIT lire l'index physiquement comme un "fantôme".
        let index_path = self
            .manager
            .storage
            .config
            .db_root(&self.manager.space, &self.manager.db)
            .join("_system.json");

        if let Ok(content) = crate::utils::io::fs::read_to_string_async(&index_path).await {
            if let Ok(index_json) =
                crate::utils::data::json::deserialize_from_str::<JsonValue>(&content)
            {
                if let Some(collections) = index_json
                    .pointer("/collections")
                    .and_then(|v| v.as_object())
                {
                    for (path, _) in collections {
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
    use crate::utils::testing::DbSandbox;

    // Mock Provider Async
    struct MockIndex {
        indexes: SharedRef<SyncMutex<UnorderedMap<String, Vec<String>>>>,
    }

    impl IndexProvider for MockIndex {
        fn has_index<'a>(&'a self, _c: &'a str, field: &'a str) -> BoxFuture<'a, bool> {
            let idx = self.indexes.clone();
            let f = field.to_string();
            Box::pin(async move {
                let guard = idx.lock().expect("Lock poisoned");
                guard.contains_key(&f)
            })
        }
        fn search<'a>(
            &'a self,
            _c: &'a str,
            field: &'a str,
            _val: &'a JsonValue,
        ) -> BoxFuture<'a, RaiseResult<Vec<String>>> {
            let idx = self.indexes.clone();
            let f = field.to_string();
            Box::pin(async move {
                let guard = idx.lock().expect("Lock poisoned");
                Ok(guard.get(&f).cloned().unwrap_or_default())
            })
        }
    }

    // 🎯 FIX : Retourne RaiseResult<()> et utilisation de `mount_points`
    #[async_test]
    async fn test_full_query_execution() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let engine = QueryEngine::new(&manager);

        manager
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        manager
            .insert_raw(
                "users",
                &json_value!({"_id": "1", "age": 20, "role": "user"}),
            )
            .await?;
        manager
            .insert_raw(
                "users",
                &json_value!({"_id": "2", "age": 30, "role": "admin"}),
            )
            .await?;

        let query = Query {
            collection: "users".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("role", json_value!("admin"))],
            }),
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        };

        let result = engine.execute_query(query).await?;
        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["_id"], "2");

        Ok(())
    }

    // 🎯 FIX : Retourne RaiseResult<()> et utilisation de `mount_points`
    #[async_test]
    async fn test_smart_like_and_array() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;

        let engine = QueryEngine::new(&manager);

        manager
            .create_collection(
                "docs",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        manager
            .insert_raw("docs", &json_value!({"_id": "1", "tags": ["rust", "code"]}))
            .await?;

        let query = Query {
            collection: "docs".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::new(
                    "tags",
                    ComparisonOperator::Like,
                    json_value!("rust"),
                )],
            }),
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        };

        let result = engine.execute_query(query).await?;
        assert_eq!(result.total_count, 1);

        Ok(())
    }

    // 🎯 FIX : Retourne RaiseResult<()> et utilisation de `mount_points`
    #[async_test]
    async fn test_query_engine_uses_mock_index() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        DbSandbox::mock_db(&manager).await?;
        manager
            .create_collection(
                "users",
                "db://_system/_system/schemas/v1/db/generic.schema.json",
            )
            .await?;

        manager
            .insert_raw("users", &json_value!({"_id": "1", "role": "admin"}))
            .await?;
        manager
            .insert_raw("users", &json_value!({"_id": "2", "role": "user"}))
            .await?;

        let mut idx_map = UnorderedMap::new();
        idx_map.insert("role".to_string(), vec!["1".to_string()]);
        let mock_provider = Box::new(MockIndex {
            indexes: SharedRef::new(SyncMutex::new(idx_map)),
        });

        let engine = QueryEngine::new(&manager).with_index_provider(mock_provider);

        let query = Query {
            collection: "users".into(),
            filter: Some(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition::eq("role", json_value!("admin"))],
            }),
            sort: None,
            limit: None,
            offset: None,
            projection: None,
        };

        let result = engine.execute_query(query).await?;

        assert_eq!(result.total_count, 1);
        assert_eq!(result.documents[0]["_id"], "1");

        Ok(())
    }

    // 🎯 FIX : Retourne RaiseResult<()> et utilisation de `mount_points`
    #[async_test]
    async fn test_evaluate_condition_logic() -> RaiseResult<()> {
        let sandbox = DbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.storage,
            &sandbox.config.mount_points.system.domain,
            &sandbox.config.mount_points.system.db,
        );
        let engine = QueryEngine::new(&manager);

        let doc = json_value!({ "age": 25, "tags": ["a", "b"] });

        assert!(engine.evaluate_condition(&doc, &Condition::gt("age", json_value!(20)), "col"));
        assert!(!engine.evaluate_condition(&doc, &Condition::lt("age", json_value!(20)), "col"));
        assert!(engine.evaluate_condition(
            &doc,
            &Condition::contains("tags", json_value!("a")),
            "col"
        ));

        Ok(())
    }
}
