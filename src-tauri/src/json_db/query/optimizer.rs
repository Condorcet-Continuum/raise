// FICHIER : src-tauri/src/json_db/query/optimizer.rs

//! Optimiseur de requêtes pour améliorer les performances

use super::{ComparisonOperator, Condition, Query, QueryFilter};

use crate::utils::prelude::*;

/// Optimiseur de requêtes
#[derive(Debug, Default)]
pub struct QueryOptimizer {
    config: OptimizerConfig,
}

/// Configuration de l'optimiseur
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    pub reorder_conditions: bool,
    pub simplify_filters: bool,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            reorder_conditions: true,
            simplify_filters: true,
        }
    }
}

impl QueryOptimizer {
    pub fn new() -> Self {
        Self {
            config: OptimizerConfig::default(),
        }
    }

    pub fn with_config(config: OptimizerConfig) -> Self {
        Self { config }
    }

    /// Optimise une requête
    pub fn optimize(&self, mut query: Query) -> Result<Query> {
        // 1. Simplifier les filtres
        if self.config.simplify_filters {
            if let Some(ref mut filter) = query.filter {
                *filter = self.simplify_filter(filter.clone())?;
            }
        }

        // 2. Réorganiser les conditions (Sélectivité)
        if self.config.reorder_conditions {
            if let Some(ref mut filter) = query.filter {
                *filter = self.reorder_conditions(filter.clone())?;
            }
        }

        // 3. Optimiser la pagination (Sanity Check)
        query = self.optimize_pagination(query)?;

        Ok(query)
    }

    fn simplify_filter(&self, filter: QueryFilter) -> Result<QueryFilter> {
        let mut simplified = filter.clone();

        // Déduplication basique
        simplified.conditions = self.deduplicate_conditions(&simplified.conditions);

        // Si filtre vide, on retourne un vecteur vide
        if simplified.conditions.is_empty() {
            simplified.conditions = vec![];
        }

        Ok(simplified)
    }

    fn reorder_conditions(&self, mut filter: QueryFilter) -> Result<QueryFilter> {
        // Trie par sélectivité estimée (plus petit score = plus sélectif/rapide = exécuté en premier)
        filter
            .conditions
            .sort_by_key(|cond| self.estimate_selectivity(cond));
        Ok(filter)
    }

    /// Estime la sélectivité (Coût) d'une condition.
    /// Plus le score est bas, plus la condition est restrictive et rapide à vérifier.
    fn estimate_selectivity(&self, condition: &Condition) -> u32 {
        match condition.operator {
            // Très sélectif (Egalité stricte)
            ComparisonOperator::Eq => 1,
            ComparisonOperator::In => 2,

            // Sélectivité moyenne (Range)
            ComparisonOperator::Gt
            | ComparisonOperator::Gte
            | ComparisonOperator::Lt
            | ComparisonOperator::Lte => 10,

            // Sélectivité faible (Texte début/fin)
            ComparisonOperator::StartsWith | ComparisonOperator::EndsWith => 20,

            // Coûteux (Scan complet ou Regex)
            ComparisonOperator::Contains
            | ComparisonOperator::Like
            | ComparisonOperator::Matches => 50,

            // Le moins sélectif (souvent tout sauf une valeur)
            ComparisonOperator::Ne => 100,
        }
    }

    fn deduplicate_conditions(&self, conditions: &[Condition]) -> Vec<Condition> {
        let mut seen = Vec::new();
        let mut unique = Vec::new();

        for condition in conditions {
            // Clé de déduplication simple (champ + operateur + valeur stringifiée)
            let key = format!(
                "{}:{:?}:{}",
                condition.field, condition.operator, condition.value
            );
            if !seen.contains(&key) {
                seen.push(key);
                unique.push(condition.clone());
            }
        }
        unique
    }

    fn optimize_pagination(&self, mut query: Query) -> Result<Query> {
        const MAX_REASONABLE_LIMIT: usize = 1000;

        // Plafonner la limite si elle est excessive
        if let Some(limit) = query.limit {
            if limit > MAX_REASONABLE_LIMIT {
                query.limit = Some(MAX_REASONABLE_LIMIT);
            }
        }

        // Mettre une limite par défaut si on a un offset mais pas de limit (éviter le scan infini)
        if query.offset.is_some() && query.limit.is_none() {
            query.limit = Some(100);
        }

        Ok(query)
    }
}

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::query::{Condition, FilterOperator, Query, QueryFilter};
    use crate::utils::json::json;

    #[test]
    fn test_optimize_reorder() {
        let optimizer = QueryOptimizer::new();

        let mut query = Query::new("users");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![
                // Coûteux (Contains -> score 50)
                Condition {
                    field: "bio".into(),
                    operator: ComparisonOperator::Contains,
                    value: json!("developer"),
                },
                // Rapide (Eq -> score 1)
                Condition {
                    field: "status".into(),
                    operator: ComparisonOperator::Eq,
                    value: json!("active"),
                },
            ],
        });

        let optimized = optimizer.optimize(query).unwrap();
        let filter = optimized.filter.unwrap();

        // L'optimiseur doit avoir mis le Eq ("status") en premier car score 1 < score 50
        assert_eq!(filter.conditions[0].field, "status");
        assert_eq!(filter.conditions[1].field, "bio");
    }

    #[test]
    fn test_deduplicate() {
        let optimizer = QueryOptimizer::new();
        let cond = Condition {
            field: "a".into(),
            operator: ComparisonOperator::Eq,
            value: json!(1),
        };
        let conditions = vec![cond.clone(), cond.clone()]; // Doublon

        let unique = optimizer.deduplicate_conditions(&conditions);
        assert_eq!(unique.len(), 1);
    }

    #[test]
    fn test_optimize_pagination() {
        let optimizer = QueryOptimizer::new();
        let mut query = Query::new("users");
        query.limit = Some(10000); // Trop grand

        let optimized = optimizer.optimize(query).unwrap();
        assert_eq!(optimized.limit, Some(1000)); // Plafonné
    }
}
