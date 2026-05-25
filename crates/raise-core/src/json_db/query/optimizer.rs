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
    pub max_page_size: usize,
    pub cost_eq: u32,
    pub cost_range: u32,
    pub cost_text: u32,
    pub cost_expensive: u32,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            reorder_conditions: true,
            simplify_filters: true,
            max_page_size: 1000,
            cost_eq: 1,
            cost_range: 10,
            cost_text: 20,
            cost_expensive: 50,
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
    pub fn optimize(&self, mut query: Query) -> RaiseResult<Query> {
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

    fn simplify_filter(&self, filter: QueryFilter) -> RaiseResult<QueryFilter> {
        let mut simplified = filter.clone();

        // Déduplication basique
        simplified.conditions = self.deduplicate_conditions(&simplified.conditions);

        // Si filtre vide, on retourne un vecteur vide
        if simplified.conditions.is_empty() {
            simplified.conditions = vec![];
        }

        Ok(simplified)
    }

    fn reorder_conditions(&self, mut filter: QueryFilter) -> RaiseResult<QueryFilter> {
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
            ComparisonOperator::Eq | ComparisonOperator::IsA => self.config.cost_eq,
            ComparisonOperator::In => 2,

            // Sélectivité moyenne (Range)
            ComparisonOperator::Gt
            | ComparisonOperator::Gte
            | ComparisonOperator::Lt
            | ComparisonOperator::Lte => self.config.cost_range,

            // Sélectivité faible (Texte début/fin)
            ComparisonOperator::StartsWith | ComparisonOperator::EndsWith => self.config.cost_text,

            // Coûteux (Scan complet ou Regex)
            ComparisonOperator::Contains
            | ComparisonOperator::Like
            | ComparisonOperator::Matches => self.config.cost_expensive,
            ComparisonOperator::AstRule => self.config.cost_expensive,
            // Le moins sélectif (souvent tout sauf une valeur)
            ComparisonOperator::Ne => 100,
        }
    }

    fn deduplicate_conditions(&self, conditions: &[Condition]) -> Vec<Condition> {
        let mut unique: Vec<Condition> = Vec::new();
        for cond in conditions {
            // ✅ Comparaison structurelle directe sans allocation
            if !unique.iter().any(|u| u == cond) {
                unique.push(cond.clone());
            }
        }
        unique
    }

    fn optimize_pagination(&self, mut query: Query) -> RaiseResult<Query> {
        let max_limit = self.config.max_page_size;

        // Plafonner la limite si elle est excessive
        if let Some(limit) = query.limit {
            if limit > max_limit {
                query.limit = Some(max_limit);
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

    #[test]
    fn test_optimize_reorder() -> RaiseResult<()> {
        let optimizer = QueryOptimizer::new();

        let mut query = Query::new("users");
        query.filter = Some(QueryFilter {
            operator: FilterOperator::And,
            conditions: vec![
                // Coûteux (Contains -> score 50)
                Condition {
                    field: "bio".into(),
                    operator: ComparisonOperator::Contains,
                    value: json_value!("developer"),
                },
                // Rapide (Eq -> score 1)
                Condition {
                    field: "status".into(),
                    operator: ComparisonOperator::Eq,
                    value: json_value!("active"),
                },
            ],
        });

        let optimized = optimizer.optimize(query).unwrap();
        let filter = optimized.filter.unwrap();

        // L'optimiseur doit avoir mis le Eq ("status") en premier car score 1 < score 50
        assert_eq!(filter.conditions[0].field, "status");
        assert_eq!(filter.conditions[1].field, "bio");
        Ok(())
    }

    #[test]
    fn test_deduplicate() -> RaiseResult<()> {
        let optimizer = QueryOptimizer::new();
        let cond = Condition {
            field: "a".into(),
            operator: ComparisonOperator::Eq,
            value: json_value!(1),
        };
        let conditions = vec![cond.clone(), cond.clone()]; // Doublon

        let unique = optimizer.deduplicate_conditions(&conditions);
        assert_eq!(unique.len(), 1);
        Ok(())
    }

    #[test]
    fn test_optimize_pagination() -> RaiseResult<()> {
        let optimizer = QueryOptimizer::new();
        let mut query = Query::new("users");
        query.limit = Some(10000); // Trop grand

        let optimized = optimizer.optimize(query).unwrap();
        assert_eq!(optimized.limit, Some(1000));
        Ok(())
    }
}
