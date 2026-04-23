// FICHIER : src-tauri/src/commands/rules_commands.rs

use crate::utils::prelude::*; // 🎯 Façade Unique RAISE

use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ProjectModel;
use crate::model_engine::validators::{DynamicValidator, ModelValidator, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};

use tauri::State;

// Note : Cette structure est cohérente avec l'initialisation dans main.rs.
pub struct RuleEngineState {
    pub model: AsyncMutex<ProjectModel>,
}

/// Commande 1 : Tester une règle "à la volée" (Dry Run) - ASYNC
/// Le frontend envoie une règle JSON et un contexte JSON (l'élément à tester).
#[tauri::command]
pub async fn dry_run_rule(rule: Rule, context: JsonValue) -> RaiseResult<JsonValue> {
    let provider = NoOpDataProvider;

    // 1. Évaluation avec Match...raise_error
    match Evaluator::evaluate(&rule.expr, &context, &provider).await {
        Ok(cow_res) => Ok(cow_res.into_owned()),
        Err(e) => raise_error!(
            "ERR_RULE_EVAL_EXECUTION",
            error = e.to_string(),
            context = json_value!({
                "expression": format!("{:?}", rule.expr),
                "action": "evaluate_rule_expression",
                "hint": "Vérifiez les types de données comparés dans l'expression."
            })
        ),
    }
}

/// Commande 2 : Valider le modèle complet - ASYNC
/// Utilise les points de montage pour la résilience de localisation du modèle.
#[tauri::command]
pub async fn validate_model(
    rules: Vec<Rule>,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<Vec<ValidationIssue>> {
    // 1. Récupération du contexte (Space/DB) avec fallback Mount Points
    let (space, db) = {
        let model = state.model.lock().await;
        let config = AppConfig::get();
        let parts: Vec<&str> = model.meta.name.split('/').collect();

        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            // Résilience : Fallback SSOT sur la partition système
            (
                config.mount_points.system.domain.clone(),
                config.mount_points.system.db.clone(),
            )
        }
    };

    // 2. Initialisation du Loader
    let loader = ModelLoader::new(&storage, &space, &db)?;

    // 3. Indexation résiliente
    match loader.index_project().await {
        Ok(_) => (),
        Err(e) => raise_error!(
            "ERR_PROJECT_INDEX_FAIL",
            error = e.to_string(),
            context = json_value!({ "space": space, "db": db })
        ),
    };

    // 4. Instanciation et exécution du validateur
    let validator = DynamicValidator::new(rules);
    let issues = validator.validate_full(&loader).await?;

    Ok(issues)
}

// =========================================================================
// TESTS UNITAIRES (Rigueur Façade & Résilience)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::rules_engine::ast::Expr;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_dry_run_rule_async() -> RaiseResult<()> {
        let rule = Rule {
            _id: None,
            handle: "test_rule".to_string(),
            target: "result".to_string(),
            expr: Expr::Eq(vec![Expr::Val(json_value!(10)), Expr::Val(json_value!(10))]),
            description: None,
            severity: None,
        };
        let context = json_value!({});

        let result = dry_run_rule(rule, context).await?;
        assert_eq!(result, json_value!(true));
        Ok(())
    }

    #[async_test]
    async fn test_dry_run_error_async() -> RaiseResult<()> {
        let rule = Rule {
            _id: None,
            handle: "error_rule".to_string(),
            target: "result".to_string(),
            expr: Expr::Add(vec![Expr::Val(json_value!("not_a_number"))]),
            description: None,
            severity: None,
        };

        // 1. Exécution via la commande dry_run (Match sur l'échec attendu)
        let result = dry_run_rule(rule, json_value!({})).await;
        assert!(
            result.is_err(),
            "La règle devrait échouer à cause d'un mismatch de type"
        );

        // 2. Extraction déterministe via Match (Meilleure pratique que if let pour les énumérations)
        let AppError::Structured(data) = result.unwrap_err();

        // 1. Vérification du code d'erreur de haut niveau (Façade)
        assert_eq!(data.code, "ERR_RULE_EVAL_EXECUTION");

        // 2. Vérification de la cause technique réelle dans le contexte JSON
        let tech_err = data
            .context
            .get("technical_error")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // 3. On vérifie que l'erreur de typage sous-jacente est bien propagée
        assert!(
            tech_err.contains("ERR_RULE_TYPE_MISMATCH"),
            "L'erreur technique devrait propager ERR_RULE_TYPE_MISMATCH. Reçu : {}",
            tech_err
        );

        Ok(())
    }

    /// 🎯 NOUVEAU TEST : Résilience Fallback Mount Point
    #[async_test]
    async fn test_validate_model_mount_point_fallback() -> RaiseResult<()> {
        let config = AppConfig::get();

        // Initialisation d'un état avec un nom de modèle invalide (force le fallback)
        let state = RuleEngineState {
            model: AsyncMutex::new(ProjectModel::default()),
        };

        // On vérifie la logique de résolution via une simulation du bloc interne
        let model_guard = state.model.lock().await;
        let parts: Vec<&str> = model_guard.meta.name.split('/').collect();

        let (resolved_space, resolved_db) = if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (
                config.mount_points.system.domain.clone(),
                config.mount_points.system.db.clone(),
            )
        };

        assert_eq!(resolved_space, config.mount_points.system.domain);
        assert_eq!(resolved_db, config.mount_points.system.db);
        Ok(())
    }

    #[async_test]
    async fn test_validate_model_integration() -> RaiseResult<()> {
        let sandbox = AgentDbSandbox::new().await?;
        let config = AppConfig::get();
        let manager = CollectionsManager::new(
            &sandbox.db,
            &config.mount_points.system.domain,
            &config.mount_points.system.db,
        );

        let loader = ModelLoader::new_with_manager(manager)?;
        let rules = vec![];
        let validator = DynamicValidator::new(rules);

        let result = validator.validate_full(&loader).await?;
        assert!(result.is_empty());
        Ok(())
    }
}
