// FICHIER : src-tauri/src/rules_engine/mod.rs

pub mod analyzer;
pub mod ast;
pub mod evaluator;
pub mod store;

pub use analyzer::Analyzer;
pub use ast::{Expr, Rule};
pub use evaluator::{DataProvider, EvalError, Evaluator, NoOpDataProvider};
pub use store::RuleStore;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;
    use serde_json::json;
    // Imports nécessaires pour le mock DB
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::json_db::storage::{JsonDbConfig, StorageEngine};
    use tempfile::tempdir;

    #[test]
    fn test_rete_light_workflow() {
        let rule_expr = Expr::Mul(vec![
            Expr::Var("item.qty".to_string()),
            Expr::Var("item.price".to_string()),
        ]);

        let dependencies = Analyzer::get_dependencies(&rule_expr);
        assert!(dependencies.contains("item.qty"));
        assert!(dependencies.contains("item.price"));

        let context = json!({
            "item": { "qty": 5, "price": 10.5 }
        });

        let provider = NoOpDataProvider;
        let result = Evaluator::evaluate(&rule_expr, &context, &provider);

        match result {
            // CORRECTION : On utilise .as_f64() sur le Cow<Value>
            Ok(val) => assert_eq!(val.as_f64(), Some(52.5)),
            Err(e) => panic!("Erreur : {}", e),
        }
    }

    #[test]
    fn test_rule_store_indexing() {
        use std::collections::HashSet;

        // SETUP MOCK DB (Le RuleStore a besoin d'un manager pour la persistance)
        let dir = tempdir().unwrap();
        let config = JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);
        let manager = CollectionsManager::new(&storage, "test_space", "test_db");
        manager.init_db().unwrap();

        let mut store = RuleStore::new(&manager);

        let r1 = Rule {
            id: "calc_total".into(),
            target: "total".into(),
            expr: Expr::Mul(vec![Expr::Var("qty".into()), Expr::Var("price".into())]),
        };

        store.register_rule("users", r1).unwrap();

        let mut changes = HashSet::new();
        changes.insert("qty".to_string());

        let impacted = store.get_impacted_rules("users", &changes);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "calc_total");
    }

    #[test]
    fn test_logic_and_comparison() {
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

        // CORRECTION : .into_owned() pour comparer avec json!()
        assert_eq!(
            Evaluator::evaluate(&rule, &ctx_kid, &provider)
                .unwrap()
                .into_owned(),
            json!("Mineur")
        );
        assert_eq!(
            Evaluator::evaluate(&rule, &ctx_adult, &provider)
                .unwrap()
                .into_owned(),
            json!("Majeur")
        );
    }
}
