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

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

// =========================================================================
// LOGIQUE DE RÉSILIENCE DU MOTEUR DE RÈGLES
// =========================================================================

/// Initialise un RuleStore de manière résiliente en utilisant les points de montage.
pub async fn initialize_rules_engine<'a>(
    manager: &'a crate::json_db::collections::manager::CollectionsManager<'a>,
) -> RaiseResult<RuleStore<'a>> {
    let config = AppConfig::get();

    // 🎯 RÉSILIENCE MOUNT POINTS : Vérification de la collection de règles système
    // Utilise le schéma générique défini dans la partition système
    let schema_uri = format!(
        "db://{}/{}/schemas/v1/db/generic.schema.json",
        config.mount_points.system.domain, config.mount_points.system.db
    );

    match manager
        .create_collection("_system_rules", &schema_uri)
        .await
    {
        Ok(_) => Ok(RuleStore::new(manager)),
        Err(e) => raise_error!(
            "ERR_RULES_ENGINE_INIT_FAIL",
            error = e.to_string(),
            context = json_value!({ "collection": "_system_rules" })
        ),
    }
}

// =========================================================================
// TESTS UNITAIRES (Conformité Façade & Résilience)
// =========================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::rules_engine::ast::Expr;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_rete_light_workflow() -> RaiseResult<()> {
        // 1. Définition de la règle : (qty * price)
        let rule_expr = Expr::Mul(vec![
            Expr::Var("item.qty".to_string()),
            Expr::Var("item.price".to_string()),
        ]);

        // 2. Analyse statique via API publique
        let deps = Analyzer::get_dependencies(&rule_expr);
        assert!(deps.contains("item.qty"));
        assert!(deps.contains("item.price"));

        // 3. Evaluation via Match
        let context = json_value!({
            "item": { "qty": 5, "price": 10.5 }
        });
        let provider = NoOpDataProvider;

        let result = match Evaluator::evaluate(&rule_expr, &context, &provider).await {
            Ok(res) => res.into_owned(),
            Err(e) => raise_error!("ERR_RULE_EVAL_FAIL", error = e.to_string()),
        };

        assert_eq!(result.as_f64(), Some(52.5));
        Ok(())
    }

    #[async_test]
    async fn test_logic_and_comparison() -> RaiseResult<()> {
        let rule = Expr::If {
            condition: Box::new(Expr::Gte(
                Box::new(Expr::Var("age".to_string())),
                Box::new(Expr::Val(json_value!(18))),
            )),
            then_branch: Box::new(Expr::Val(json_value!("Majeur"))),
            else_branch: Box::new(Expr::Val(json_value!("Mineur"))),
        };

        let ctx_kid = json_value!({ "age": 12 });
        let provider = NoOpDataProvider;

        let res = Evaluator::evaluate(&rule, &ctx_kid, &provider).await?;
        assert_eq!(res.as_str(), Some("Mineur"));
        Ok(())
    }

    #[async_test]
    async fn test_rule_store_indexing_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        let config = AppConfig::get();

        // 🎯 RÉSILIENCE MOUNT POINTS : Utilisation dynamique de la config système
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        // Initialisation via la nouvelle façade résiliente
        let mut store = initialize_rules_engine(&manager).await?;

        let rule = Rule {
            id: "calc_total".into(),
            target: "total".into(),
            expr: Expr::Mul(vec![Expr::Var("qty".into()), Expr::Var("price".into())]),
            description: None,
            severity: None,
        };

        // Enregistrement
        match store.register_rule("invoices", rule).await {
            Ok(_) => (),
            Err(e) => raise_error!("ERR_RULE_REGISTRATION_FAIL", error = e.to_string()),
        }

        // Simulation changement sur "qty"
        let mut changes = UniqueSet::new();
        changes.insert("qty".to_string());

        let impacted = store.get_impacted_rules("invoices", &changes);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "calc_total");

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience face à un échec de point de montage
    #[async_test]
    async fn test_rules_engine_mount_point_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await;
        // Simulation d'une partition fantôme
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        let result = initialize_rules_engine(&manager).await;

        // Le moteur doit intercepter l'échec de création de collection proprement
        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_RULES_ENGINE_INIT_FAIL");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_RULES_ENGINE_INIT_FAIL"),
        }
    }

    /// 🎯 NOUVEAU TEST : Inférence des dépendances complexes
    #[test]
    fn test_analyzer_complex_dependencies() {
        let expr = Expr::Add(vec![
            Expr::Var("a".into()),
            Expr::Mul(vec![Expr::Var("b".into()), Expr::Val(json_value!(2))]),
        ]);
        let deps = Analyzer::get_dependencies(&expr);
        assert!(deps.contains("a"));
        assert!(deps.contains("b"));
        assert_eq!(deps.len(), 2);
    }
}
