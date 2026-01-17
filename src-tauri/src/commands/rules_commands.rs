// FICHIER : src-tauri/src/commands/rules_commands.rs

use crate::model_engine::types::ProjectModel;
use crate::model_engine::validators::{DynamicValidator, ModelValidator, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use serde_json::Value;
// CORRECTION E0277 : Utilisation du Mutex de Tokio pour garantir que le MutexGuard est Send
// et peut être conservé à travers un point d'attente .await.
use tauri::State;
use tokio::sync::Mutex;

// Note : Cette structure doit être cohérente avec l'initialisation dans main.rs.
pub struct RuleEngineState {
    pub model: Mutex<ProjectModel>,
}

/// Commande 1 : Tester une règle "à la volée" (Dry Run) - ASYNC
/// Le frontend envoie une règle JSON et un contexte JSON (l'élément à tester).
/// Le backend renvoie le résultat (True/False ou valeur calculée).
#[tauri::command]
pub async fn dry_run_rule(rule: Rule, context: Value) -> Result<Value, String> {
    let provider = NoOpDataProvider;

    // On évalue la règle sans persistance (Correction: ajout de .await car l'Evaluator est async)
    match Evaluator::evaluate(&rule.expr, &context, &provider).await {
        Ok(cow_res) => Ok(cow_res.into_owned()),
        Err(e) => Err(format!("Erreur d'évaluation : {}", e)),
    }
}

/// Commande 2 : Valider le modèle complet - ASYNC
/// Le frontend envoie la liste des règles actives.
/// Le backend verrouille le modèle, le valide, et renvoie la liste des problèmes.
#[tauri::command]
pub async fn validate_model(
    rules: Vec<Rule>,
    state: State<'_, RuleEngineState>, // Utilisation du lifetime anonyme pour State
) -> Result<Vec<ValidationIssue>, String> {
    // 1. Accès sécurisé au modèle en mémoire (Verrouillage asynchrone)
    // CORRECTION E0277 : On utilise .await sur le verrou Tokio qui est compatible avec le trait Send.
    // Contrairement à std::sync::Mutex, le verrou de Tokio ne retourne pas de Result (pas de poisoning).
    let model_guard = state.model.lock().await;

    // 2. Instanciation du validateur dynamique
    let validator = DynamicValidator::new(rules);

    // 3. Exécution
    // CORRECTION E0308 : La méthode validate est asynchrone (async_trait).
    // On utilise .await pour obtenir Vec<ValidationIssue>.
    let issues = validator.validate(&model_guard).await;

    Ok(issues)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;
    use serde_json::json;

    #[tokio::test] // Utilisation de tokio pour les tests asynchrones
    async fn test_dry_run_rule_async() {
        let rule = Rule {
            id: "test_rule".to_string(),
            target: "result".to_string(),
            expr: Expr::Eq(vec![Expr::Val(json!(10)), Expr::Val(json!(10))]),
            description: None,
            severity: None,
        };
        let context = json!({});

        let result = dry_run_rule(rule, context).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), json!(true));
    }

    #[tokio::test]
    async fn test_dry_run_error_async() {
        let rule = Rule {
            id: "error_rule".to_string(),
            target: "result".to_string(),
            expr: Expr::Add(vec![Expr::Val(json!("not_a_number"))]),
            description: None,
            severity: None,
        };

        let result = dry_run_rule(rule, json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Type incompatible"));
    }

    #[tokio::test]
    async fn test_validate_model_async() {
        // Initialisation de l'état avec le Mutex de Tokio
        let state = RuleEngineState {
            model: Mutex::new(ProjectModel::default()),
        };

        let rules = vec![];
        let validator = DynamicValidator::new(rules);

        // Verrouillage asynchrone dans le test
        let model = state.model.lock().await;
        let result = validator.validate(&model).await;
        assert!(result.is_empty());
    }
}
