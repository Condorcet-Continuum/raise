// FICHIER : src-tauri/src/rules_engine/evaluator.rs
use crate::rules_engine::ast::Expr;
use crate::utils::{async_trait, prelude::*, DateTime, Regex, Utc};
use chrono::{Duration, NaiveDate};

use std::borrow::Cow;

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("Champ introuvable : {0}")]
    VarNotFound(String),
    #[error("Type incompatible : attendu nombre")]
    NotANumber,
    #[error("Type incompatible : attendu chaîne de caractères")]
    NotAString,
    #[error("Type incompatible : attendu tableau")]
    NotAnArray,
    #[error("Format de date invalide (attendu ISO8601/RFC3339) : {0}")]
    InvalidDate(String),
    #[error("Erreur Regex : {0}")]
    InvalidRegex(String),
    #[error("Erreur générique : {0}")]
    Generic(String),
}

// 🎯 NOUVEAU : Conversion automatique pour que les autres modules
// puissent faire "Evaluator::evaluate(...)?" sans se soucier du type d'erreur !
impl From<EvalError> for crate::utils::error::AppError {
    fn from(err: EvalError) -> Self {
        crate::utils::error::AppError::Validation(format!(
            "Erreur d'évaluation des règles : {}",
            err
        ))
    }
}

/// Trait permettant aux règles d'accéder à des données externes (Lookups)
#[async_trait]
pub trait DataProvider: Send + Sync {
    async fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<Value>;
}

pub struct NoOpDataProvider;
#[async_trait]
impl DataProvider for NoOpDataProvider {
    async fn get_value(&self, _c: &str, _id: &str, _f: &str) -> Option<Value> {
        None
    }
}

pub struct Evaluator;

impl Evaluator {
    // 🎯 CORRECTION : Utilisation de std::result::Result pour accepter 2 paramètres (Succès, EvalError)
    pub async fn evaluate<'a>(
        expr: &'a Expr,
        context: &'a Value,
        provider: &dyn DataProvider,
    ) -> std::result::Result<Cow<'a, Value>, EvalError> {
        match expr {
            Expr::Val(v) => Ok(Cow::Borrowed(v)),
            Expr::Var(path) => resolve_path(context, path),

            // --- Opérateurs Logiques ---
            Expr::And(list) => {
                for e in list {
                    let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                    if !is_truthy(&val) {
                        return Ok(Cow::Owned(Value::Bool(false)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(true)))
            }
            Expr::Or(list) => {
                for e in list {
                    let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                    if is_truthy(&val) {
                        return Ok(Cow::Owned(Value::Bool(true)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(false)))
            }
            Expr::Not(e) => {
                let res = Box::pin(Self::evaluate(e, context, provider)).await?;
                Ok(Cow::Owned(Value::Bool(!is_truthy(&res))))
            }

            // --- Comparaisons ---
            Expr::Eq(args) => {
                if args.len() < 2 {
                    return Ok(Cow::Owned(Value::Bool(true)));
                }
                let first = Box::pin(Self::evaluate(&args[0], context, provider)).await?;
                for arg in &args[1..] {
                    let next = Box::pin(Self::evaluate(arg, context, provider)).await?;
                    if first != next {
                        return Ok(Cow::Owned(Value::Bool(false)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(true)))
            }
            Expr::Neq(args) => {
                if args.len() < 2 {
                    return Ok(Cow::Owned(Value::Bool(false)));
                }
                let a = Box::pin(Self::evaluate(&args[0], context, provider)).await?;
                let b = Box::pin(Self::evaluate(&args[1], context, provider)).await?;
                Ok(Cow::Owned(Value::Bool(a != b)))
            }
            Expr::Gt(a, b) => compare_nums(a, b, context, provider, |x, y| x > y).await,
            Expr::Lt(a, b) => compare_nums(a, b, context, provider, |x, y| x < y).await,
            Expr::Gte(a, b) => compare_nums(a, b, context, provider, |x, y| x >= y).await,
            Expr::Lte(a, b) => compare_nums(a, b, context, provider, |x, y| x <= y).await,

            // --- Mathématiques ---
            Expr::Add(list) => fold_nums(list, context, provider, 0.0, |acc, x| acc + x).await,
            Expr::Mul(list) => fold_nums(list, context, provider, 1.0, |acc, x| acc * x).await,
            Expr::Sub(list) => {
                if list.is_empty() {
                    return Ok(Cow::Owned(json!(0)));
                }
                let mut acc = Box::pin(Self::evaluate(&list[0], context, provider))
                    .await?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                for e in &list[1..] {
                    acc -= Box::pin(Self::evaluate(e, context, provider))
                        .await?
                        .as_f64()
                        .ok_or(EvalError::NotANumber)?;
                }
                Ok(Cow::Owned(smart_number(acc)))
            }
            Expr::Div(list) => {
                if list.len() < 2 {
                    return Err(EvalError::Generic("Div requiert au moins 2 args".into()));
                }
                let num = Box::pin(Self::evaluate(&list[0], context, provider))
                    .await?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                let den = Box::pin(Self::evaluate(&list[1], context, provider))
                    .await?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                if den == 0.0 {
                    return Err(EvalError::Generic("Division par zéro".into()));
                }
                Ok(Cow::Owned(smart_number(num / den)))
            }
            Expr::Abs(e) => {
                let v = Box::pin(Self::evaluate(e, context, provider))
                    .await?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                Ok(Cow::Owned(smart_number(v.abs())))
            }
            Expr::Round { value, precision } => {
                let v = Box::pin(Self::evaluate(value, context, provider))
                    .await?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                let p = Box::pin(Self::evaluate(precision, context, provider))
                    .await?
                    .as_i64()
                    .unwrap_or(0);
                let factor = 10f64.powi(p as i32);
                let res = (v * factor).round() / factor;
                Ok(Cow::Owned(smart_number(res)))
            }

            // CORRECTIF : Ajout Min/Max pour test_list_aggregations
            Expr::Min(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let arr = val.as_array().ok_or(EvalError::NotAnArray)?;

                let min = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(f64::INFINITY, |a, b| a.min(b));

                if min.is_infinite() {
                    Ok(Cow::Owned(Value::Null))
                } else {
                    Ok(Cow::Owned(smart_number(min)))
                }
            }
            Expr::Max(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let arr = val.as_array().ok_or(EvalError::NotAnArray)?;

                let max = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(f64::NEG_INFINITY, |a, b| a.max(b));

                if max.is_infinite() {
                    Ok(Cow::Owned(Value::Null))
                } else {
                    Ok(Cow::Owned(smart_number(max)))
                }
            }

            // --- Collections & Itérations ---
            Expr::Map {
                list,
                alias,
                expr: map_expr,
            } => {
                let list_val = Box::pin(Self::evaluate(list, context, provider)).await?;
                let arr = list_val.as_array().ok_or(EvalError::NotAnArray)?;

                let mut result_arr = Vec::new();
                for item in arr {
                    let mut local_ctx = context.clone();
                    if let Some(obj) = local_ctx.as_object_mut() {
                        obj.insert(alias.clone(), item.clone());
                    }
                    let res = Box::pin(Self::evaluate(map_expr, &local_ctx, provider)).await?;
                    result_arr.push(res.into_owned());
                }
                Ok(Cow::Owned(Value::Array(result_arr)))
            }
            Expr::Filter {
                list,
                alias,
                condition,
            } => {
                let list_val = Box::pin(Self::evaluate(list, context, provider)).await?;
                let arr = list_val.as_array().ok_or(EvalError::NotAnArray)?;

                let mut result_arr = Vec::new();
                for item in arr {
                    let mut local_ctx = context.clone();
                    if let Some(obj) = local_ctx.as_object_mut() {
                        obj.insert(alias.clone(), item.clone());
                    }
                    let cond_res =
                        Box::pin(Self::evaluate(condition, &local_ctx, provider)).await?;
                    if is_truthy(&cond_res) {
                        result_arr.push(item.clone());
                    }
                }
                Ok(Cow::Owned(Value::Array(result_arr)))
            }

            // --- String & Regex ---
            Expr::RegexMatch { value, pattern } => {
                let v_str = Box::pin(Self::evaluate(value, context, provider)).await?;
                let p_str = Box::pin(Self::evaluate(pattern, context, provider)).await?;
                let v = v_str.as_str().ok_or(EvalError::NotAString)?;
                let p = p_str.as_str().ok_or(EvalError::NotAString)?;
                let re = Regex::new(p).map_err(|e| EvalError::InvalidRegex(e.to_string()))?;
                Ok(Cow::Owned(Value::Bool(re.is_match(v))))
            }
            Expr::Concat(list) => {
                let mut res = String::new();
                for e in list {
                    let v = Box::pin(Self::evaluate(e, context, provider)).await?;
                    res.push_str(v.as_str().unwrap_or(&v.to_string()));
                }
                Ok(Cow::Owned(Value::String(res)))
            }

            // CORRECTIF : Implémentation de Replace pour test_string_extensions
            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                let v_val = Box::pin(Self::evaluate(value, context, provider)).await?;
                let p_val = Box::pin(Self::evaluate(pattern, context, provider)).await?;
                let r_val = Box::pin(Self::evaluate(replacement, context, provider)).await?;

                let v = v_val.as_str().ok_or(EvalError::NotAString)?;
                let p = p_val.as_str().ok_or(EvalError::NotAString)?;
                let r = r_val.as_str().ok_or(EvalError::NotAString)?;

                Ok(Cow::Owned(Value::String(v.replace(p, r))))
            }

            Expr::Upper(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let s = val.as_str().ok_or(EvalError::NotAString)?;
                Ok(Cow::Owned(Value::String(s.to_uppercase())))
            }
            Expr::Lower(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let s = val.as_str().ok_or(EvalError::NotAString)?;
                Ok(Cow::Owned(Value::String(s.to_lowercase())))
            }
            Expr::Trim(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let s = val.as_str().ok_or(EvalError::NotAString)?;
                Ok(Cow::Owned(Value::String(s.trim().to_string())))
            }

            Expr::Len(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let len = match val.as_ref() {
                    Value::Array(arr) => arr.len(),
                    Value::String(s) => s.chars().count(),
                    Value::Object(obj) => obj.len(),
                    _ => 0,
                };
                Ok(Cow::Owned(json!(len)))
            }

            // --- Structure de Contrôle ---
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let val_cond = Box::pin(Self::evaluate(condition, context, provider)).await?;
                if is_truthy(&val_cond) {
                    Box::pin(Self::evaluate(then_branch, context, provider)).await
                } else {
                    Box::pin(Self::evaluate(else_branch, context, provider)).await
                }
            }

            // --- Dates ---
            Expr::Now => Ok(Cow::Owned(json!(Utc::now().to_rfc3339()))),
            Expr::DateAdd { date, days } => {
                let d_val = Box::pin(Self::evaluate(date, context, provider)).await?;
                let days_val = Box::pin(Self::evaluate(days, context, provider))
                    .await?
                    .as_i64()
                    .unwrap_or(0);
                let d_str = d_val.as_str().ok_or(EvalError::NotAString)?;
                if let Ok(dt) = DateTime::parse_from_rfc3339(d_str) {
                    Ok(Cow::Owned(json!(
                        (dt + Duration::days(days_val)).to_rfc3339()
                    )))
                } else if let Ok(nd) = NaiveDate::parse_from_str(d_str, "%Y-%m-%d") {
                    Ok(Cow::Owned(json!((nd + Duration::days(days_val))
                        .format("%Y-%m-%d")
                        .to_string())))
                } else {
                    Err(EvalError::InvalidDate(d_str.to_string()))
                }
            }

            // --- Lookup (ASYNCHRONE) ---
            Expr::Lookup {
                collection,
                id,
                field,
            } => {
                let id_v = Box::pin(Self::evaluate(id, context, provider)).await?;
                let id_s = id_v.as_str().unwrap_or("");
                let res = provider
                    .get_value(collection, id_s, field)
                    .await
                    .unwrap_or(Value::Null);
                Ok(Cow::Owned(res))
            }

            // Catch-all
            _ => Ok(Cow::Owned(Value::Null)),
        }
    }
}

// --- Helpers ---

// 🎯 CORRECTION : Idem ici, on remplace RaiseResult par std::result::Result
async fn compare_nums<'a, F>(
    a: &Expr,
    b: &Expr,
    c: &'a Value,
    p: &dyn DataProvider,
    op: F,
) -> std::result::Result<Cow<'a, Value>, EvalError>
where
    F: Fn(f64, f64) -> bool,
{
    let va = Box::pin(Evaluator::evaluate(a, c, p))
        .await?
        .as_f64()
        .ok_or(EvalError::NotANumber)?;
    let vb = Box::pin(Evaluator::evaluate(b, c, p))
        .await?
        .as_f64()
        .ok_or(EvalError::NotANumber)?;
    Ok(Cow::Owned(Value::Bool(op(va, vb))))
}

async fn fold_nums<'a, F>(
    list: &[Expr],
    c: &'a Value,
    p: &dyn DataProvider,
    init: f64,
    op: F,
) -> std::result::Result<Cow<'a, Value>, EvalError>
where
    F: Fn(f64, f64) -> f64,
{
    let mut acc = init;
    for e in list {
        let val = Box::pin(Evaluator::evaluate(e, c, p))
            .await?
            .as_f64()
            .ok_or(EvalError::NotANumber)?;
        acc = op(acc, val);
    }
    Ok(Cow::Owned(smart_number(acc)))
}

fn smart_number(n: f64) -> Value {
    if n.fract() == 0.0 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

fn resolve_path<'a>(
    context: &'a Value,
    path: &str,
) -> std::result::Result<Cow<'a, Value>, EvalError> {
    let mut current = context;
    if path.is_empty() {
        return Ok(Cow::Borrowed(current));
    }
    for part in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map
                    .get(part)
                    .ok_or_else(|| EvalError::VarNotFound(path.to_string()))?
            }
            _ => return Err(EvalError::Generic("Path resolution failed".into())),
        }
    }
    Ok(Cow::Borrowed(current))
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        Value::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn test_eq_async() {
        let provider = NoOpDataProvider;
        let ctx = json!({});
        let expr = Expr::Eq(vec![Expr::Val(json!(10)), Expr::Val(json!(10))]);
        let res = Evaluator::evaluate(&expr, &ctx, &provider).await.unwrap();
        assert_eq!(res.as_bool(), Some(true));
    }

    #[tokio::test]
    async fn test_lookup_mock() {
        struct MockProvider;
        #[async_trait]
        impl DataProvider for MockProvider {
            async fn get_value(&self, _c: &str, _id: &str, _f: &str) -> Option<Value> {
                Some(json!("Alice"))
            }
        }
        let expr = Expr::Lookup {
            collection: "users".into(),
            id: Box::new(Expr::Val(json!("u1"))),
            field: "name".into(),
        };
        let context_data = json!({});
        let res = Evaluator::evaluate(&expr, &context_data, &MockProvider)
            .await
            .unwrap();
        assert_eq!(res.as_str(), Some("Alice"));
    }
}
