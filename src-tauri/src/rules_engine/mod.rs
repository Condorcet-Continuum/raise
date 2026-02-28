// FICHIER : src-tauri/src/rules_engine/mod.rs

pub mod analyzer;
pub mod ast;
pub mod evaluator;
pub mod store;

// Exports publics
pub use analyzer::Analyzer;
pub use ast::{Expr, Rule};
pub use evaluator::{DataProvider, Evaluator, NoOpDataProvider};
pub use store::RuleStore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;
    use crate::utils::data::json;

    // Imports nécessaires pour les tests d'intégration du Store
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use std::collections::HashSet;
    use tempfile::tempdir;

    #[tokio::test]
    async fn test_rete_light_workflow() {
        // 1. Définition de la règle : (qty * price)
        let rule_expr = Expr::Mul(vec![
            Expr::Var("item.qty".to_string()),
            Expr::Var("item.price".to_string()),
        ]);

        // CORRECTION E0063 : Ajout des champs description et severity
        let _r1 = Rule {
            id: "calc_total".to_string(),
            target: "total".to_string(),
            expr: rule_expr.clone(),
            description: None,
            severity: None,
        };

        // 2. Analyse statique
        // CORRECTION E0624/E0308 : Utilisation de l'API publique get_dependencies
        let deps = Analyzer::get_dependencies(&rule_expr);

        assert!(deps.contains("item.qty"));
        assert!(deps.contains("item.price"));

        // 3. Evaluation
        let context = json!({
            "item": { "qty": 5, "price": 10.5 }
        });
        let provider = NoOpDataProvider;

        let result = Evaluator::evaluate(&rule_expr, &context, &provider)
            .await
            .unwrap();
        assert_eq!(result.as_f64(), Some(52.5));
    }

    #[tokio::test]
    async fn test_logic_and_comparison() {
        // Règle : Si age >= 18 alors "Majeur" sinon "Mineur"
        let rule = Expr::If {
            condition: Box::new(Expr::Gte(
                Box::new(Expr::Var("age".to_string())),
                Box::new(Expr::Val(json!(18))),
            )),
            then_branch: Box::new(Expr::Val(json!("Majeur"))),
            else_branch: Box::new(Expr::Val(json!("Mineur"))),
        };

        let ctx_kid = json!({ "age": 12 });
        let ctx_adult = json!({ "age": 25 });
        let provider = NoOpDataProvider;

        Evaluator::evaluate(&rule, &ctx_kid, &provider)
            .await
            .unwrap();
        Evaluator::evaluate(&rule, &ctx_adult, &provider)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_rule_store_indexing() {
        // Setup de l'environnement DB temporaire
        crate::utils::config::test_mocks::inject_mock_config();
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        manager.init_db().await.unwrap();

        let mut store = RuleStore::new(&manager);

        // Règle : dépend de "qty"
        let rule = Rule {
            id: "calc_total".into(),
            target: "total".into(),
            expr: Expr::Mul(vec![Expr::Var("qty".into()), Expr::Var("price".into())]),
            description: None,
            severity: None,
        };

        // Enregistrement
        store.register_rule("invoices", rule).await.unwrap();

        // Simulation changement sur "qty"
        let mut changes = HashSet::new();
        changes.insert("qty".to_string());

        // Vérification : La règle doit être récupérée
        let impacted = store.get_impacted_rules("invoices", &changes);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "calc_total");

        // Simulation changement sur "other" (non utilisé)
        let mut changes_irrelevant = HashSet::new();
        changes_irrelevant.insert("other_field".to_string());

        let impacted_none = store.get_impacted_rules("invoices", &changes_irrelevant);
        assert!(impacted_none.is_empty());
    }
}
