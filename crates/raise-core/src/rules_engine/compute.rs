// FICHIER : src-tauri/src/rules_engine/compute.rs

use crate::rules_engine::{Evaluator, Expr, NoOpDataProvider};
use crate::utils::prelude::*;

/// Le contexte d'exécution pur (DTO) pour le moteur x_compute.
/// Zéro lifetime complexe, 100% thread-safe.
#[derive(Clone, Debug, Default)]
pub struct ComputeContext {
    pub document: JsonValue,
    pub collection_name: String,
    pub db_name: String,
    pub space_name: String,
}
/* ToDo
#[derive(Clone, Debug, Default)]
pub struct ComputeContext {
    pub document: JsonValue,
    // 🎯 Un conteneur dynamique pour toutes les variables d'environnement/contexte
    pub vars: JsonObject<String, JsonValue>,
}
    ....//  RÉSOLUTION DYNAMIQUE dans get/get_context: On cherche d'abord dans les variables système
        if let Some(val) = context.vars.get(path) {
            return Ok(val.clone());
        }
*/
/// Signature claire et stricte pour le registre.
pub type AsyncComputeFn =
    for<'a> fn(
        plan: JsonObject<String, JsonValue>,
        context: &'a ComputeContext,
    ) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>>;

// --- 1. OPÉRATEURS PRIMITIFS ---
// On retire #[async_recursive] et on utilise Box::pin pour forcer la signature

pub fn compute_uuid_v4<'a>(
    _plan: JsonObject<String, JsonValue>,
    _ctx: &'a ComputeContext,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move { Ok(json_value!(UniqueId::new_v4().to_string())) })
}

pub fn compute_now_rfc3339<'a>(
    _plan: JsonObject<String, JsonValue>,
    _ctx: &'a ComputeContext,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move { Ok(json_value!(UtcClock::now().to_rfc3339())) })
}

pub fn compute_const<'a>(
    plan: JsonObject<String, JsonValue>,
    _ctx: &'a ComputeContext,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move { Ok(plan.get("value").cloned().unwrap_or(JsonValue::Null)) })
}

// --- 2. OPÉRATEURS DE RÉSOLUTION ---

pub fn compute_get_context<'a>(
    plan: JsonObject<String, JsonValue>,
    context: &'a ComputeContext,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move {
        let path = plan.get("path").and_then(|v| v.as_str()).unwrap_or("");

        match path {
            "database_name" => return Ok(json_value!(context.db_name.clone())),
            "collection_name" => return Ok(json_value!(context.collection_name.clone())),
            "domain_name" | "space_name" => return Ok(json_value!(context.space_name.clone())),
            _ => {}
        };

        match crate::rules_engine::evaluator::resolve_path(&context.document, path) {
            Ok(res) => Ok(res.into_owned()),
            Err(_) => Ok(JsonValue::Null),
        }
    })
}

pub fn compute_expr<'a>(
    plan: JsonObject<String, JsonValue>,
    context: &'a ComputeContext,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move {
        let expr_val = plan.get("expression").ok_or_else(|| {
            build_error!(
                "ERR_COMPUTE_MISSING_EXPR",
                error = "Le plan 'expr' nécessite une expression AST."
            )
        })?;

        let expr: Expr = json::deserialize_from_value(expr_val.clone())?;
        let provider = NoOpDataProvider;
        let res = Evaluator::evaluate(&expr, &context.document, &provider).await?;

        Ok(res.into_owned())
    })
}

pub fn compute_concat<'a>(
    plan: JsonObject<String, JsonValue>,
    context: &'a ComputeContext,
) -> Pinned<Box<dyn AsyncFuture<Output = RaiseResult<JsonValue>> + Send + 'a>> {
    Box::pin(async move {
        let args = plan.get("args").and_then(|v| v.as_array()).ok_or_else(|| {
            build_error!(
                "ERR_COMPUTE_INVALID_CONCAT",
                error = "L'opérateur 'concat' requiert un tableau 'args'."
            )
        })?;

        let mut result = String::new();

        for arg in args {
            if let Some(s) = arg.as_str() {
                result.push_str(s);
            } else if let Some(sub_plan) = arg.as_object() {
                if let Some(op) = sub_plan.get("op").and_then(|v| v.as_str()) {
                    // C'est ici que la véritable récursion se fait !
                    let val = execute_compute_plan(op, sub_plan, context).await?;
                    result.push_str(val.as_str().unwrap_or(""));
                }
            }
        }

        Ok(json_value!(result))
    })
}

// --- 3. REGISTRE ET EXÉCUTION ---

pub const COMPUTE_REGISTRY: &[(&str, AsyncComputeFn)] = &[
    ("uuid_v4", compute_uuid_v4),
    ("now_rfc3339", compute_now_rfc3339),
    ("const", compute_const),
    ("get", compute_get_context),
    ("get_context", compute_get_context),
    ("expr", compute_expr),
    ("concat", compute_concat),
];

pub async fn execute_compute_plan(
    op: &str,
    plan: &JsonObject<String, JsonValue>,
    context: &ComputeContext,
) -> RaiseResult<JsonValue> {
    for &(registered_op, func) in COMPUTE_REGISTRY {
        if registered_op == op {
            return func(plan.clone(), context).await;
        }
    }

    user_warn!(
        "WARN_COMPUTE_OP_UNKNOWN",
        json_value!({ "operator": op, "hint": "L'opérateur x_compute est inconnu." })
    );

    Ok(JsonValue::Null)
}

// --- 4. TESTS ---
#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_compute_semantic_iri_generation() -> RaiseResult<()> {
        let context = ComputeContext {
            document: json_value!({ "handle": "devops-engineer" }),
            db_name: "system".to_string(),
            collection_name: "actors".to_string(),
            space_name: "master".to_string(),
        };

        let plan = json_value!({
            "op": "concat",
            "args": [
                "db://",
                { "op": "get_context", "path": "space_name" },
                "/",
                { "op": "get_context", "path": "database_name" },
                "/collections/",
                { "op": "get_context", "path": "collection_name" },
                "/handle/",
                { "op": "get", "path": "handle" }
              ]
        });

        let op = plan["op"].as_str().unwrap();
        let res = execute_compute_plan(op, plan.as_object().unwrap(), &context).await?;

        assert_eq!(
            res,
            json_value!("db://master/system/collections/actors/handle/devops-engineer")
        );
        Ok(())
    }

    #[async_test]
    async fn test_compute_deep_interpolation() -> RaiseResult<()> {
        let context = ComputeContext {
            document: json_value!({
                "env": "prod",
                "project": { "code": "condorcet", "version": "v2" },
                "entity": { "handle": "thermal-sensor-01" }
            }),
            ..Default::default()
        };

        let plan = json_value!({
            "op": "concat",
            "args": [
                "db://",
                { "op": "get_context", "path": "env" },
                "/",
                {
                    "op": "concat",
                    "args": [
                        { "op": "get_context", "path": "project.code" },
                        "/",
                        {
                            "op": "expr",
                            "expression": {
                                "concat": [
                                    { "var": "entity.handle" },
                                    { "val": "--" },
                                    { "var": "project.version" }
                                ]
                            }
                        }
                    ]
                }
            ]
        });

        let res = execute_compute_plan("concat", plan.as_object().unwrap(), &context).await?;
        assert_eq!(
            res,
            json_value!("db://prod/condorcet/thermal-sensor-01--v2")
        );
        Ok(())
    }
}
