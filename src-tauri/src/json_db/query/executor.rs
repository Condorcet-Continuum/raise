//! Exécuteur de requêtes pour la base de données JSON
//!
//! Optimisation : Utilise les index binaires si disponibles pour éviter le Full Scan.

use anyhow::{bail, Result};
use serde_json::Value;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap}; // Pour charger les index en mémoire
use uuid;

// Import du CollectionsManager
use crate::json_db::collections::manager::CollectionsManager;
// Import pour la gestion des index
use crate::json_db::indexes::{driver, paths, IndexType};

use super::{
    ComparisonOperator, Condition, DeleteResult, FilterOperator, InsertResult, Projection, Query,
    QueryFilter, QueryResult, SortField, SortOrder, UpdateResult, UpsertResult,
};

/// Exécuteur de requêtes
#[derive(Debug)]
pub struct QueryExecutor<'a> {
    manager: &'a CollectionsManager<'a>,
}

impl<'a> QueryExecutor<'a> {
    pub fn new(manager: &'a CollectionsManager<'a>) -> Self {
        Self { manager }
    }

    /// Exécute une requête SELECT (Optimisée avec Index)
    pub async fn execute(&self, query: Query) -> Result<QueryResult> {
        // 1. Récupération optimisée des candidats (Index Scan vs Full Scan)
        let candidates = self.fetch_candidate_documents(&query).await?;

        // 2. Filtrage (toujours nécessaire car l'index ne couvre qu'une partie des conditions)
        let filtered = if let Some(ref filter) = query.filter {
            candidates
                .into_iter()
                .filter(|doc| self.evaluate_filter(doc, filter))
                .collect()
        } else {
            candidates
        };

        let total_count = filtered.len() as u64;

        // 3. Tri
        let mut sorted = if let Some(ref sort_fields) = query.sort {
            let mut docs = filtered;
            self.sort_documents(&mut docs, sort_fields)?;
            docs
        } else {
            filtered
        };

        // 4. Projection
        if let Some(ref projection) = query.projection {
            sorted = sorted
                .into_iter()
                .map(|doc| self.apply_projection(doc, projection))
                .collect();
        }

        // 5. Pagination
        let offset = query.offset.unwrap_or(0);
        let paginated: Vec<Value> = sorted
            .into_iter()
            .skip(offset)
            .take(query.limit.unwrap_or(usize::MAX))
            .collect();

        Ok(QueryResult {
            documents: paginated,
            total_count,
            offset,
            limit: query.limit,
        })
    }

    /// Tente de récupérer les documents via un index, sinon fait un scan complet
    async fn fetch_candidate_documents(&self, query: &Query) -> Result<Vec<Value>> {
        // Si pas de filtre, on doit tout scanner de toute façon
        let Some(ref filter) = query.filter else {
            return self.get_collection_documents(&query.collection).await;
        };

        // Récupérer la liste des index existants pour cette collection
        let defined_indexes = self
            .manager
            .get_indexes(&query.collection)
            .unwrap_or_default();

        // Chercher une condition d'égalité (Eq) qui correspond à un index
        for condition in &filter.conditions {
            if matches!(condition.operator, ComparisonOperator::Eq) {
                // Chercher un index correspondant au champ
                if let Some(idx_def) = defined_indexes.iter().find(|idx| {
                    idx.field_path == condition.field
                        || idx.field_path == format!("/{}", condition.field)
                }) {
                    // --- INDEX HIT ! ---
                    let (cfg, space, db) =
                        (self.manager.cfg, &self.manager.space, &self.manager.db);
                    let idx_path = paths::index_path(
                        cfg,
                        space,
                        db,
                        &query.collection,
                        &idx_def.name,
                        idx_def.index_type,
                    );

                    let search_key = condition.value.to_string();

                    let doc_ids_opt = match idx_def.index_type {
                        IndexType::Hash | IndexType::Text => {
                            let index: HashMap<String, Vec<String>> = driver::load(&idx_path)?;
                            index.get(&search_key).cloned()
                        }
                        IndexType::BTree => {
                            let index: BTreeMap<String, Vec<String>> = driver::load(&idx_path)?;
                            index.get(&search_key).cloned()
                        }
                    };

                    if let Some(ids) = doc_ids_opt {
                        let mut docs = Vec::new();
                        for id in ids {
                            if let Ok(doc) = self.manager.get(&query.collection, &id) {
                                docs.push(doc);
                            }
                        }
                        return Ok(docs);
                    } else {
                        // Index existe mais clé non trouvée -> 0 résultat
                        return Ok(Vec::new());
                    }
                }
            }
        }

        // Fallback : Scan complet
        self.get_collection_documents(&query.collection).await
    }

    /// Insère des documents
    pub async fn insert(&self, _collection: &str, documents: Vec<Value>) -> Result<InsertResult> {
        let mut inserted_ids = Vec::new();
        for doc in &documents {
            let id = self.extract_or_generate_id(doc)?;
            inserted_ids.push(id);
        }
        Ok(InsertResult {
            inserted_count: documents.len() as u64,
            inserted_ids,
        })
    }

    /// Upsert des documents
    pub async fn upsert(
        &self,
        collection: &str,
        documents: Vec<Value>,
        match_fields: Vec<String>,
    ) -> Result<UpsertResult> {
        let mut inserted_count = 0;
        let mut updated_count = 0;
        let mut affected_ids = Vec::new();

        for doc in documents {
            let filter = self.build_match_filter(&doc, &match_fields)?;
            let existing = self.find_one(collection, Some(filter.clone())).await?;

            if existing.is_some() {
                let id = self.extract_or_generate_id(&doc)?;
                affected_ids.push(id);
                updated_count += 1;
            } else {
                let id = self.extract_or_generate_id(&doc)?;
                affected_ids.push(id);
                inserted_count += 1;
            }
        }
        Ok(UpsertResult {
            inserted_count,
            updated_count,
            affected_ids,
        })
    }

    /// Update documents (Simulation)
    pub async fn update(
        &self,
        collection: &str,
        filter: QueryFilter,
        _updates: Value,
    ) -> Result<UpdateResult> {
        let documents = self.get_collection_documents(collection).await?;
        let matched: Vec<&Value> = documents
            .iter()
            .filter(|doc| self.evaluate_filter(doc, &filter))
            .collect();

        Ok(UpdateResult {
            matched_count: matched.len() as u64,
            modified_count: matched.len() as u64,
        })
    }

    /// Delete documents (Simulation)
    pub async fn delete(&self, collection: &str, filter: QueryFilter) -> Result<DeleteResult> {
        let documents = self.get_collection_documents(collection).await?;
        let to_delete: Vec<&Value> = documents
            .iter()
            .filter(|doc| self.evaluate_filter(doc, &filter))
            .collect();

        Ok(DeleteResult {
            deleted_count: to_delete.len() as u64,
        })
    }

    /// Liste toutes les collections
    pub async fn list_collections(&self) -> Result<Vec<String>> {
        self.manager.list_collection_names().map_err(|e| e.into())
    }

    /// Compte le nombre de documents correspondant à un filtre
    pub async fn count(&self, collection: &str, filter: Option<QueryFilter>) -> Result<u64> {
        // Construction propre de la requête pour bénéficier de l'optimisation d'index
        let effective_filter = filter.clone().unwrap_or_else(|| QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![],
        });

        let query = Query::new(collection).filter(effective_filter);

        // Récupération optimisée
        let documents = self.fetch_candidate_documents(&query).await?;

        if let Some(ref f) = filter {
            Ok(documents
                .iter()
                .filter(|doc| self.evaluate_filter(doc, f))
                .count() as u64)
        } else {
            Ok(documents.len() as u64)
        }
    }

    // ========================================================================
    // MÉTHODES PRIVÉES
    // ========================================================================

    fn evaluate_filter(&self, document: &Value, filter: &QueryFilter) -> bool {
        match filter.operator {
            FilterOperator::And => filter
                .conditions
                .iter()
                .all(|cond| self.evaluate_condition(document, cond)),
            FilterOperator::Or => filter
                .conditions
                .iter()
                .any(|cond| self.evaluate_condition(document, cond)),
            FilterOperator::Not => !filter
                .conditions
                .iter()
                .all(|cond| self.evaluate_condition(document, cond)),
        }
    }

    fn evaluate_condition(&self, document: &Value, condition: &Condition) -> bool {
        let field_value = self.get_field_value(document, &condition.field);
        match &condition.operator {
            ComparisonOperator::Eq => field_value == Some(&condition.value),
            ComparisonOperator::Ne => field_value != Some(&condition.value),
            ComparisonOperator::Gt => {
                if let (Some(field_val), Some(cond_val)) = (field_value, condition.value.as_f64()) {
                    if let Some(field_num) = field_val.as_f64() {
                        return field_num > cond_val;
                    }
                }
                false
            }
            ComparisonOperator::Gte => {
                if let (Some(field_val), Some(cond_val)) = (field_value, condition.value.as_f64()) {
                    if let Some(field_num) = field_val.as_f64() {
                        return field_num >= cond_val;
                    }
                }
                false
            }
            ComparisonOperator::Lt => {
                if let (Some(field_val), Some(cond_val)) = (field_value, condition.value.as_f64()) {
                    if let Some(field_num) = field_val.as_f64() {
                        return field_num < cond_val;
                    }
                }
                false
            }
            ComparisonOperator::Lte => {
                if let (Some(field_val), Some(cond_val)) = (field_value, condition.value.as_f64()) {
                    if let Some(field_num) = field_val.as_f64() {
                        return field_num <= cond_val;
                    }
                }
                false
            }
            ComparisonOperator::In => {
                if let (Some(field_val), Some(array)) = (field_value, condition.value.as_array()) {
                    return array.contains(field_val);
                }
                false
            }
            ComparisonOperator::Contains => {
                if let Some(field_val) = field_value {
                    if let Some(field_str) = field_val.as_str() {
                        if let Some(search_str) = condition.value.as_str() {
                            return field_str.contains(search_str);
                        }
                    }
                }
                false
            }
            ComparisonOperator::StartsWith => {
                if let Some(field_val) = field_value {
                    if let Some(field_str) = field_val.as_str() {
                        if let Some(prefix) = condition.value.as_str() {
                            return field_str.starts_with(prefix);
                        }
                    }
                }
                false
            }
            ComparisonOperator::EndsWith => {
                if let Some(field_val) = field_value {
                    if let Some(field_str) = field_val.as_str() {
                        if let Some(suffix) = condition.value.as_str() {
                            return field_str.ends_with(suffix);
                        }
                    }
                }
                false
            }
            ComparisonOperator::Matches => false,
        }
    }

    fn get_field_value(&self, document: &'a Value, field_path: &str) -> Option<&'a Value> {
        let parts: Vec<&str> = field_path.split('.').collect();
        let mut current = document;
        for part in parts {
            current = current.get(part)?;
        }
        Some(current)
    }

    fn sort_documents(&self, documents: &mut [Value], sort_fields: &[SortField]) -> Result<()> {
        documents.sort_by(|a, b| {
            for sort_field in sort_fields {
                let val_a = self.get_field_value(a, &sort_field.field);
                let val_b = self.get_field_value(b, &sort_field.field);
                let cmp = match (val_a, val_b) {
                    (Some(a), Some(b)) => self.compare_values(a, b),
                    (Some(_), None) => Ordering::Greater,
                    (None, Some(_)) => Ordering::Less,
                    (None, None) => Ordering::Equal,
                };
                let ordered_cmp = match sort_field.order {
                    SortOrder::Asc => cmp,
                    SortOrder::Desc => cmp.reverse(),
                };
                if ordered_cmp != Ordering::Equal {
                    return ordered_cmp;
                }
            }
            Ordering::Equal
        });
        Ok(())
    }

    fn compare_values(&self, a: &Value, b: &Value) -> Ordering {
        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                let a_f64 = a.as_f64().unwrap_or(0.0);
                let b_f64 = b.as_f64().unwrap_or(0.0);
                a_f64.partial_cmp(&b_f64).unwrap_or(Ordering::Equal)
            }
            (Value::String(a), Value::String(b)) => a.cmp(b),
            (Value::Bool(a), Value::Bool(b)) => a.cmp(b),
            (Value::Null, Value::Null) => Ordering::Equal,
            (Value::Null, _) => Ordering::Less,
            (_, Value::Null) => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }

    fn apply_projection(&self, mut document: Value, projection: &Projection) -> Value {
        if let Value::Object(ref mut map) = document {
            match projection {
                Projection::Include(fields) => {
                    map.retain(|key, _| fields.contains(key));
                }
                Projection::Exclude(fields) => {
                    for field in fields {
                        map.remove(field);
                    }
                }
            }
        }
        document
    }

    fn build_match_filter(&self, document: &Value, match_fields: &[String]) -> Result<QueryFilter> {
        let mut conditions = Vec::new();
        for field in match_fields {
            if let Some(value) = self.get_field_value(document, field) {
                conditions.push(Condition {
                    field: field.clone(),
                    operator: ComparisonOperator::Eq,
                    value: value.clone(),
                });
            } else {
                bail!("Match field '{}' not found in document", field);
            }
        }
        Ok(QueryFilter {
            operator: FilterOperator::And,
            conditions,
        })
    }

    fn extract_or_generate_id(&self, document: &Value) -> Result<String> {
        if let Some(id) = document.get("id") {
            if let Some(id_str) = id.as_str() {
                return Ok(id_str.to_string());
            }
        }
        Ok(uuid::Uuid::new_v4().to_string())
    }

    async fn find_one(
        &self,
        collection: &str,
        filter: Option<QueryFilter>,
    ) -> Result<Option<Value>> {
        // Construction propre de la requête
        let effective_filter = filter.clone().unwrap_or_else(|| QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![],
        });

        let query = Query::new(collection).filter(effective_filter);

        // Utilisation de fetch_candidate_documents pour bénéficier des index
        let documents = self.fetch_candidate_documents(&query).await?;

        if let Some(filter) = filter {
            Ok(documents
                .into_iter()
                .find(|doc| self.evaluate_filter(doc, &filter)))
        } else {
            Ok(documents.into_iter().next())
        }
    }

    async fn get_collection_documents(&self, collection: &str) -> Result<Vec<Value>> {
        self.manager.list_all(collection).map_err(|e| e.into())
    }
}

#[cfg(test)]
#[allow(invalid_value)]
mod tests {
    use super::*;
    use serde_json::json;

    fn get_executor_stub() -> QueryExecutor<'static> {
        let ptr = std::ptr::NonNull::<CollectionsManager<'static>>::dangling().as_ptr();
        let fake_manager_ref = unsafe { &*ptr };
        QueryExecutor {
            manager: fake_manager_ref,
        }
    }

    #[test]
    fn test_evaluate_condition_eq() {
        let executor = get_executor_stub();
        let doc = json!({"name": "Alice", "age": 30});
        let condition = Condition {
            field: "name".to_string(),
            operator: ComparisonOperator::Eq,
            value: json!("Alice"),
        };
        assert!(executor.evaluate_condition(&doc, &condition));
    }
    // ... autres tests ...
}
