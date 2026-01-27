// FICHIER : src-tauri/src/commands/rules_commands.rs

use crate::json_db::storage::StorageEngine;
use crate::model_engine::loader::ModelLoader;
use crate::model_engine::types::ProjectModel;
use crate::model_engine::validators::{DynamicValidator, ModelValidator, ValidationIssue};
use crate::rules_engine::ast::Rule;
use crate::rules_engine::evaluator::{Evaluator, NoOpDataProvider};
use serde_json::Value;
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

    // On évalue la règle sans persistance
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
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>, // Injection du moteur de stockage
) -> Result<Vec<ValidationIssue>, String> {
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
    loader.index_project().await.map_err(|e| e.to_string())?;

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
    use crate::rules_engine::ast::Expr;
    use serde_json::json;
    use tempfile::tempdir; // Requis pour simuler le stockage

    #[tokio::test]
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
    async fn test_validate_model_integration() {
        // Pour tester validate_model (qui utilise le loader), il faut une vraie DB temporaire

        // 1. Setup DB
        let dir = tempdir().unwrap();
        let config = crate::json_db::storage::JsonDbConfig::new(dir.path().to_path_buf());
        let storage = StorageEngine::new(config);

        let manager = crate::json_db::collections::manager::CollectionsManager::new(
            &storage,
            "test_space",
            "test_db",
        );
        manager.init_db().await.unwrap();

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
