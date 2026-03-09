// FICHIER : src-tauri/src/commands/rules_commands.rs

use crate::utils::prelude::*;

use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ProjectModel;
use crate::model_engine::validators::{DynamicValidator, ModelValidator, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};

use tauri::State;

// Note : Cette structure doit être cohérente avec l'initialisation dans main.rs.
pub struct RuleEngineState {
    pub model: AsyncMutex<ProjectModel>,
}

/// Commande 1 : Tester une règle "à la volée" (Dry Run) - ASYNC
/// Le frontend envoie une règle JSON et un contexte JSON (l'élément à tester).
/// Le backend renvoie le résultat (True/False ou valeur calculée).
#[tauri::command]
pub async fn dry_run_rule(rule: Rule, context: JsonValue) -> RaiseResult<JsonValue> {
    let provider = NoOpDataProvider;

    // 1. On évalue la règle sans persistance
    let result = match Evaluator::evaluate(&rule.expr, &context, &provider).await {
        Ok(cow_res) => cow_res.into_owned(),
        Err(e) => raise_error!(
            "ERR_RULE_EVAL_EXECUTION",
            error = e,
            context = json_value!({
                "expression": format!("{:?}", rule.expr),
                "action": "evaluate_rule_expression",
                "hint": "Erreur de syntaxe ou variable manquante dans le contexte. Vérifiez les types de données comparés."
            })
        ),
    };

    // 2. On retourne le résultat brut
    Ok(result)
}

/// Commande 2 : Valider le modèle complet - ASYNC
/// Le frontend envoie la liste des règles actives.
/// Le backend verrouille le modèle, le valide, et renvoie la liste des problèmes.
#[tauri::command]
pub async fn validate_model(
    rules: Vec<Rule>,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>, // Injection du moteur de stockage
) -> RaiseResult<Vec<ValidationIssue>> {
    // 1. Récupération du contexte (Space/DB) depuis le modèle en mémoire
    // On a besoin de savoir "où" nous sommes pour initialiser le Loader
    let (space, db) = {
        let model = state.model.lock().await;
        let parts: Vec<&str> = model.meta.name.split('/').collect();
        if parts.len() >= 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            // Valeurs par défaut si le modèle n'est pas encore chargé
            ("default".to_string(), "default".to_string())
        }
    }; // Le lock est relâché ici

    // 2. Initialisation du Loader (Mode Lazy)
    let loader = ModelLoader::new(&storage, &space, &db);

    // On indexe rapidement les fichiers pour permettre les lookups
    // On décompose pour un contrôle total sur l'indexation
    let _project_index = match loader.index_project().await {
        Ok(index) => index,
        Err(e) => raise_error!(
            "ERR_PROJECT_INDEX_FAIL",
            error = e,
            context = json_value!({
                "action": "index_project_structure",
                "loader_state": "active",
                "hint": "L'indexeur n'a pas pu scanner le projet. Vérifiez les permissions de lecture ou la présence d'un fichier de configuration corrompu à la racine."
            })
        ),
    };

    // 3. Instanciation du validateur dynamique
    let validator = DynamicValidator::new(rules);

    // 4. Exécution de la validation globale via le Loader
    // C'est ici que la magie opère : validate_full va utiliser le loader pour
    // charger les éléments nécessaires et exécuter les règles.
    let issues = validator.validate_full(&loader).await;

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::collections::manager::CollectionsManager;
    use crate::rules_engine::ast::Expr;
    use crate::utils::testing::AgentDbSandbox;

    #[async_test]
    async fn test_dry_run_rule_async() {
        let rule = Rule {
            id: "test_rule".to_string(),
            target: "result".to_string(),
            expr: Expr::Eq(vec![Expr::Val(json_value!(10)), Expr::Val(json_value!(10))]),
            description: None,
            severity: None,
        };
        let context = json_value!({});

        let result = dry_run_rule(rule, context).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json_value!(true));
    }

    #[async_test]
    async fn test_dry_run_error_async() {
        let rule = Rule {
            id: "error_rule".to_string(),
            target: "result".to_string(),
            expr: Expr::Add(vec![Expr::Val(json_value!("not_a_number"))]),
            description: None,
            severity: None,
        };

        let result = dry_run_rule(rule, json_value!({})).await;
        assert!(result.is_err());

        let err = result.unwrap_err();

        // On déstructure l'erreur de la commande (le wrapper)
        let AppError::Structured(data) = err;

        // 1. La commande signale un échec global d'exécution
        assert_eq!(data.code, "ERR_RULE_EVAL_EXECUTION");

        // 2. 🎯 FIX : On vérifie que la CAUSE technique est bien un mismatch de type
        let tech_err = data
            .context
            .get("technical_error")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        assert!(
            tech_err.contains("ERR_RULE_TYPE_MISMATCH"),
            "L'erreur devrait propager le code technique de mismatch. Reçu : {}",
            tech_err
        );
    }

    #[async_test]
    async fn test_validate_model_integration() {
        let sandbox = AgentDbSandbox::new().await;
        let manager = CollectionsManager::new(
            &sandbox.db,
            &sandbox.config.system_domain,
            &sandbox.config.system_db,
        );

        // 2. Setup Loader
        let loader = ModelLoader::new_with_manager(manager);

        // 3. Setup Validator
        let rules = vec![];
        let validator = DynamicValidator::new(rules);

        // 4. Exécution (On teste directement la logique interne car mocker 'State' est complexe)
        let result = validator.validate_full(&loader).await;
        assert!(result.is_empty());
    }
}
