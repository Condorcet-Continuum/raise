// FICHIER : crates/raise-desktop/src/commands/rules_commands.rs

use raise_core::json_db::storage::StorageEngine;
use raise_core::model_engine::validators::ValidationIssue;
use raise_core::rules_engine::ast::Rule;
use raise_core::utils::prelude::*;

// 🎯 On importe le service pur depuis le noyau
use raise_core::services::rules_service::{self, RuleEngineState};

use tauri::State;

#[tauri::command]
pub async fn dry_run_rule(rule: Rule, context: JsonValue) -> RaiseResult<JsonValue> {
    rules_service::dry_run_rule(rule, context).await
}

#[tauri::command]
pub async fn validate_model(
    rules: Vec<Rule>,
    state: State<'_, RuleEngineState>,
    storage: State<'_, StorageEngine>,
) -> RaiseResult<Vec<ValidationIssue>> {
    rules_service::validate_model(rules, state.inner(), storage.inner()).await
}
