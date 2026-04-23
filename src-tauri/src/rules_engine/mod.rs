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

use crate::json_db::collections::data_provider::CachedDataProvider;
use crate::json_db::collections::manager::CollectionsManager;
use crate::json_db::schema::SchemaRegistry;
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
    let schema_uri = format!(
        "db://{}/{}/schemas/v2/assurance/rules/rule.schema.json",
        config.mount_points.system.domain, config.mount_points.system.db
    );

    if let Err(e) = manager
        .create_collection("_system_rules", &schema_uri)
        .await
    {
        if !e.to_string().contains("ERR_DB_COLLECTION_ALREADY_EXISTS") {
            raise_error!(
                "ERR_RULES_ENGINE_INIT_FAIL",
                error = e.to_string(),
                context = json_value!({
                    "collection": "_system_rules",
                    "db": format!("{}/{}", manager.space, manager.db)
                })
            );
        }
    }
    Ok(RuleStore::new(manager))
}

#[async_recursive]
pub async fn apply_business_rules(
    manager: &CollectionsManager<'_>,
    collection_name: &str,
    doc: &mut JsonValue,
    old_doc: Option<&JsonValue>,
    registry: &SchemaRegistry,
    schema_uri: &str,
) -> RaiseResult<()> {
    // 🛡️ LOOP GUARD (Zéro Dette) : On ne vérifie pas les règles pour les collections système.
    if collection_name == "_system_rules" || collection_name == "_system_index" {
        return Ok(());
    }

    // On utilise l'initialiseur résilient
    let mut store = initialize_rules_engine(manager).await?;

    // 🎯 CORRECTIF DML : On se contente de synchroniser depuis le disque (Lecture seule)
    // AUCUNE opération I/O d'écriture n'est tolérée ici.
    store.sync_from_db().await?;

    // 1. Extraction des règles "in-line" du schéma
    if let Some(schema) = registry.get_by_uri(schema_uri) {
        if let Some(rules_array) = schema.get("x_rules").and_then(|v| v.as_array()) {
            for rule_val in rules_array.iter() {
                if let Ok(rule) = json::deserialize_from_value::<Rule>(rule_val.clone()) {
                    // 🎯 FIX E0616 : Délégation propre au RuleStore (Encapsulation parfaite)
                    let _ = store.cache_inline_rule(collection_name, rule);
                }
            }
        }
    }

    // 2. Cycle d'évaluation (RETE-light)
    let provider = CachedDataProvider::new(manager.storage, &manager.space, &manager.db);
    let mut current_changes = compute_diff(doc, old_doc);
    let mut passes = 0;
    const MAX_PASSES: usize = 10;

    while !current_changes.is_empty() && passes < MAX_PASSES {
        let rules = store.get_impacted_rules(collection_name, &current_changes);
        if rules.is_empty() {
            break;
        }

        let mut next_changes = UniqueSet::new();
        for rule in rules {
            if let Ok(result) = Evaluator::evaluate(&rule.expr, doc, &provider).await {
                if set_value_by_path(doc, &rule.target, result.into_owned()) {
                    next_changes.insert(rule.target.clone());
                }
            }
        }
        current_changes = next_changes;
        passes += 1;
    }
    Ok(())
}

// =========================================================================
// HELPERS DML (Diff & Set)
// =========================================================================

fn compute_diff(new_doc: &JsonValue, old_doc: Option<&JsonValue>) -> UniqueSet<String> {
    let mut changes = UniqueSet::new();
    let mut path_stack = Vec::new();
    find_changes(&mut path_stack, new_doc, old_doc, &mut changes);
    changes
}

fn find_changes<'a>(
    path_stack: &mut Vec<&'a str>,
    new_val: &'a JsonValue,
    old_val: Option<&JsonValue>,
    changes: &mut UniqueSet<String>,
) {
    if let Some(old) = old_val {
        if new_val == old {
            return;
        }
    }
    if !path_stack.is_empty() {
        changes.insert(path_stack.join("."));
    }

    match (new_val, old_val) {
        (JsonValue::Object(new_map), Some(JsonValue::Object(old_map))) => {
            for (k, v) in new_map {
                path_stack.push(k.as_str());
                find_changes(path_stack, v, old_map.get(k), changes);
                path_stack.pop();
            }
        }
        (JsonValue::Object(new_map), None) => {
            for (k, v) in new_map {
                path_stack.push(k.as_str());
                find_changes(path_stack, v, None, changes);
                path_stack.pop();
            }
        }
        _ => {}
    }
}

fn set_value_by_path(doc: &mut JsonValue, path: &str, value: JsonValue) -> bool {
    let parts: Vec<&str> = path.split('.').collect();
    let mut current = doc;

    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let Some(obj) = current.as_object_mut() {
                if obj.get(*part) != Some(&value) {
                    obj.insert(part.to_string(), value);
                    return true;
                }
            }
        } else {
            if !current.is_object() {
                *current = json_value!({});
            }
            if current.get(*part).is_none() {
                current
                    .as_object_mut()
                    .unwrap()
                    .insert(part.to_string(), json_value!({}));
            }
            current = current.get_mut(*part).unwrap();
        }
    }
    false
}

// =========================================================================
// TESTS UNITAIRES (Conformité Façade & Résilience)
// =========================================================================
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
        let rule_expr = Expr::Mul(vec![
            Expr::Var("item.qty".to_string()),
            Expr::Var("item.price".to_string()),
        ]);

        let deps = Analyzer::get_dependencies(&rule_expr, 50)?;
        assert!(deps.contains("item.qty"));
        assert!(deps.contains("item.price"));

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
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();

        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let mut store = initialize_rules_engine(&manager).await?;

        let rule = Rule {
            _id: None,
            handle: "calc_total".into(),
            target: "total".into(),
            expr: Expr::Mul(vec![Expr::Var("qty".into()), Expr::Var("price".into())]),
            description: None,
            severity: None,
        };

        // 🎯 1. DML Asynchrone
        let finalized_rule = match store.save_rule_document("invoices", rule).await {
            Ok(r) => r,
            Err(e) => raise_error!("ERR_RULE_REGISTRATION_FAIL", error = e.to_string()),
        };

        // 🎯 2. Indexation DDL Synchrone avec le Jeton
        {
            let lock = manager
                .storage
                .get_index_lock(&manager.space, &manager.db)?;
            let guard = lock.lock().await;
            let mut tx = manager.begin_system_tx(&guard).await?;

            if let Err(e) = store.promote_rule_to_index("invoices", finalized_rule, &mut tx) {
                raise_error!("ERR_RULE_PROMOTION_FAIL", error = e.to_string());
            }

            tx.commit().await?;
        }

        let mut changes = UniqueSet::new();
        changes.insert("qty".to_string());

        let impacted = store.get_impacted_rules("invoices", &changes);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].handle, "calc_total");

        Ok(())
    }

    #[async_test]
    async fn test_rules_engine_mount_point_resilience() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let manager = CollectionsManager::new(&sandbox.db, "ghost_partition", "void_db");

        let result = initialize_rules_engine(&manager).await;

        match result {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_RULES_ENGINE_INIT_FAIL");
                Ok(())
            }
            _ => panic!("Le moteur aurait dû lever ERR_RULES_ENGINE_INIT_FAIL"),
        }
    }

    #[async_test]
    async fn test_analyzer_complex_dependencies() -> RaiseResult<()> {
        let expr = Expr::Add(vec![
            Expr::Var("a".into()),
            Expr::Mul(vec![Expr::Var("b".into()), Expr::Val(json_value!(2))]),
        ]);

        let deps = Analyzer::get_dependencies(&expr, 50)?;

        assert!(deps.contains("a"));
        assert!(deps.contains("b"));
        assert_eq!(deps.len(), 2);
        Ok(())
    }
}
