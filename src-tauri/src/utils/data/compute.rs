// FICHIER : src-tauri/src/utils/data/compute.rs

use crate::user_warn;
use crate::utils::core::{CalendarDuration, UniqueId, UtcClock};
use crate::utils::data::json::{json_value, JsonObject, JsonValue};

/// Signature standardisée pour tous les opérateurs de calcul sémantique
pub type ComputeOperatorFn = fn(&JsonObject<String, JsonValue>) -> JsonValue;

fn compute_uuid_v4(_plan: &JsonObject<String, JsonValue>) -> JsonValue {
    JsonValue::String(UniqueId::new_v4().to_string())
}

fn compute_now_rfc3339(_plan: &JsonObject<String, JsonValue>) -> JsonValue {
    JsonValue::String(UtcClock::now().to_rfc3339())
}

fn compute_now_plus_hours(plan: &JsonObject<String, JsonValue>) -> JsonValue {
    let hours = plan
        .get("value")
        .and_then(|v: &JsonValue| v.as_i64())
        .unwrap_or_default();
    let future = UtcClock::now() + CalendarDuration::hours(hours);
    JsonValue::String(future.to_rfc3339())
}

fn compute_now_plus_days(plan: &JsonObject<String, JsonValue>) -> JsonValue {
    let days = plan
        .get("value")
        .and_then(|v: &JsonValue| v.as_i64())
        .unwrap_or_default();
    let future = UtcClock::now() + CalendarDuration::days(days);
    JsonValue::String(future.to_rfc3339())
}

fn compute_const(plan: &JsonObject<String, JsonValue>) -> JsonValue {
    plan.get("value").cloned().unwrap_or(JsonValue::Null)
}

/// 🎯 Registre statique public des opérations sémantiques
pub const COMPUTE_REGISTRY: &[(&str, ComputeOperatorFn)] = &[
    ("uuid_v4", compute_uuid_v4),
    ("now_rfc3339", compute_now_rfc3339),
    ("now_plus_hours", compute_now_plus_hours),
    ("now_plus_days", compute_now_plus_days),
    ("const", compute_const),
];

/// Évalue dynamiquement un plan de calcul (utilisable par le Validator, le RulesEngine ou les Agents IA)
pub fn execute_compute_plan(op: &str, plan: &JsonObject<String, JsonValue>) -> JsonValue {
    for &(registered_op, func) in COMPUTE_REGISTRY {
        if registered_op == op {
            return func(plan);
        }
    }

    user_warn!(
        "WARN_COMPUTE_OP_UNKNOWN",
        json_value!({
            "operator": op,
            "plan": plan,
            "available_operators": COMPUTE_REGISTRY.iter().map(|(k, _)| *k).collect::<Vec<_>>(),
            "hint": "Opérateur x_compute inconnu ou non pris en charge ignoré."
        })
    );

    JsonValue::Null
}
