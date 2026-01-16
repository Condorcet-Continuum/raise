// FICHIER : src-tauri/src/commands/rules.rs

use crate::model_engine::types::ProjectModel;
use crate::model_engine::validators::{DynamicValidator, ModelValidator, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use serde_json::Value;
use std::sync::Mutex;
use tauri::State;

// Note : Assurez-vous que cette structure (ou une structure similaire contenant le modèle)
// est bien gérée dans votre main.rs via .manage()
pub struct RuleEngineState {
    pub model: Mutex<ProjectModel>,
}

/// Commande 1 : Tester une règle "à la volée" (Dry Run)
/// Le frontend envoie une règle JSON et un contexte JSON (l'élément à tester).
/// Le backend renvoie le résultat (True/False ou valeur calculée).
#[tauri::command]
pub fn dry_run_rule(rule: Rule, context: Value) -> Result<Value, String> {
    let provider = NoOpDataProvider;

    // On évalue la règle sans persistance
    match Evaluator::evaluate(&rule.expr, &context, &provider) {
        Ok(cow_res) => Ok(cow_res.into_owned()),
        Err(e) => Err(format!("Erreur d'évaluation : {}", e)),
    }
}

/// Commande 2 : Valider le modèle complet
/// Le frontend envoie la liste des règles actives.
/// Le backend verrouille le modèle, le valide, et renvoie la liste des problèmes.
#[tauri::command]
pub fn validate_model(
    rules: Vec<Rule>,
    state: State<RuleEngineState>,
) -> Result<Vec<ValidationIssue>, String> {
    // 1. Accès sécurisé au modèle en mémoire
    let model_guard = state
        .model
        .lock()
        .map_err(|_| "Impossible de verrouiller le modèle")?;

    // 2. Instanciation du validateur dynamique
    let validator = DynamicValidator::new(rules);

    // 3. Exécution
    let issues = validator.validate(&model_guard);

    Ok(issues)
}
