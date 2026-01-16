// FICHIER : src-tauri/src/rules_engine/evaluator.rs

use crate::rules_engine::ast::Expr;
use chrono::{DateTime, Duration, NaiveDate, Utc};
use regex::Regex;
use serde_json::{json, Value};
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

pub trait DataProvider {
    fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<Value>;
}

pub struct NoOpDataProvider;
impl DataProvider for NoOpDataProvider {
    fn get_value(&self, _c: &str, _id: &str, _f: &str) -> Option<Value> {
        None
    }
}

pub struct Evaluator;

impl Evaluator {
    pub fn evaluate<'a>(
        expr: &'a Expr,
        context: &'a Value,
        provider: &dyn DataProvider,
    ) -> Result<Cow<'a, Value>, EvalError> {
        match expr {
            Expr::Val(v) => Ok(Cow::Borrowed(v)),
            Expr::Var(path) => resolve_path(context, path),

            // --- Opérateurs Logiques ---
            Expr::And(list) => {
                for e in list {
                    let val = Self::evaluate(e, context, provider)?;
                    if !is_truthy(&val) {
                        return Ok(Cow::Owned(Value::Bool(false)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(true)))
            }
            Expr::Or(list) => {
                for e in list {
                    let val = Self::evaluate(e, context, provider)?;
                    if is_truthy(&val) {
                        return Ok(Cow::Owned(Value::Bool(true)));
                    }
                }
                Ok(Cow::Owned(Value::Bool(false)))
            }
            Expr::Not(e) => {
                let res = Self::evaluate(e, context, provider)?;
                Ok(Cow::Owned(Value::Bool(!is_truthy(&res))))
            }

            // --- Comparaisons ---
            Expr::Eq(args) => {
                if args.len() < 2 {
                    return Ok(Cow::Owned(Value::Bool(true)));
                }
                let first = Self::evaluate(&args[0], context, provider)?;
                for arg in &args[1..] {
                    let next = Self::evaluate(arg, context, provider)?;
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
                let a = Self::evaluate(&args[0], context, provider)?;
                let b = Self::evaluate(&args[1], context, provider)?;
                Ok(Cow::Owned(Value::Bool(a != b)))
            }
            Expr::Gt(a, b) => compare_nums(a, b, context, provider, |x, y| x > y),
            Expr::Lt(a, b) => compare_nums(a, b, context, provider, |x, y| x < y),
            Expr::Gte(a, b) => compare_nums(a, b, context, provider, |x, y| x >= y),
            Expr::Lte(a, b) => compare_nums(a, b, context, provider, |x, y| x <= y),

            // --- Mathématiques ---
            Expr::Add(list) => fold_nums(list, context, provider, 0.0, |acc, x| acc + x),
            Expr::Mul(list) => fold_nums(list, context, provider, 1.0, |acc, x| acc * x),
            Expr::Sub(list) => {
                if list.is_empty() {
                    return Ok(Cow::Owned(json!(0)));
                }
                let mut acc = Self::evaluate(&list[0], context, provider)?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                for e in &list[1..] {
                    acc -= Self::evaluate(e, context, provider)?
                        .as_f64()
                        .ok_or(EvalError::NotANumber)?;
                }
                Ok(Cow::Owned(smart_number(acc)))
            }
            Expr::Div(list) => {
                if list.len() < 2 {
                    return Err(EvalError::Generic("Div requiert au moins 2 args".into()));
                }
                let num = Self::evaluate(&list[0], context, provider)?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                let den = Self::evaluate(&list[1], context, provider)?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                if den == 0.0 {
                    return Err(EvalError::Generic("Division par zéro".into()));
                }
                Ok(Cow::Owned(smart_number(num / den)))
            }
            Expr::Abs(e) => {
                let v = Self::evaluate(e, context, provider)?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                Ok(Cow::Owned(smart_number(v.abs())))
            }
            Expr::Round { value, precision } => {
                let v = Self::evaluate(value, context, provider)?
                    .as_f64()
                    .ok_or(EvalError::NotANumber)?;
                let p = Self::evaluate(precision, context, provider)?
                    .as_i64()
                    .unwrap_or(0);
                let factor = 10f64.powi(p as i32);
                let res = (v * factor).round() / factor;
                Ok(Cow::Owned(smart_number(res)))
            }

            // --- Collections & Itérations ---
            Expr::Map {
                list,
                alias,
                expr: map_expr,
            } => {
                let list_val = Self::evaluate(list, context, provider)?;
                let arr = list_val.as_array().ok_or(EvalError::NotAnArray)?;

                let mut result_arr = Vec::new();
                for item in arr {
                    let mut local_ctx = context.clone();
                    if let Some(obj) = local_ctx.as_object_mut() {
                        obj.insert(alias.clone(), item.clone());
                    } else {
                        local_ctx = json!({ alias: item });
                    }

                    let res = Self::evaluate(map_expr, &local_ctx, provider)?;
                    result_arr.push(res.into_owned());
                }
                Ok(Cow::Owned(Value::Array(result_arr)))
            }
            Expr::Filter {
                list,
                alias,
                condition,
            } => {
                let list_val = Self::evaluate(list, context, provider)?;
                let arr = list_val.as_array().ok_or(EvalError::NotAnArray)?;

                let mut result_arr = Vec::new();
                for item in arr {
                    let mut local_ctx = context.clone();
                    if let Some(obj) = local_ctx.as_object_mut() {
                        obj.insert(alias.clone(), item.clone());
                    } else {
                        local_ctx = json!({ alias: item });
                    }

                    let cond_res = Self::evaluate(condition, &local_ctx, provider)?;
                    if is_truthy(&cond_res) {
                        result_arr.push(item.clone());
                    }
                }
                Ok(Cow::Owned(Value::Array(result_arr)))
            }

            // --- String & Regex ---
            Expr::RegexMatch { value, pattern } => {
                let v_str = Self::evaluate(value, context, provider)?;
                let p_str = Self::evaluate(pattern, context, provider)?;

                let v = v_str.as_str().ok_or(EvalError::NotAString)?;
                let p = p_str.as_str().ok_or(EvalError::NotAString)?;

                let re = Regex::new(p).map_err(|e| EvalError::InvalidRegex(e.to_string()))?;
                Ok(Cow::Owned(Value::Bool(re.is_match(v))))
            }
            Expr::Trim(e) => {
                let v = Self::evaluate(e, context, provider)?;
                Ok(Cow::Owned(Value::String(
                    v.as_str().unwrap_or("").trim().to_string(),
                )))
            }
            Expr::Lower(e) => {
                let v = Self::evaluate(e, context, provider)?;
                Ok(Cow::Owned(Value::String(
                    v.as_str().unwrap_or("").to_lowercase(),
                )))
            }
            Expr::Upper(e) => {
                let v = Self::evaluate(e, context, provider)?;
                Ok(Cow::Owned(Value::String(
                    v.as_str().unwrap_or("").to_uppercase(),
                )))
            }
            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                let v = Self::evaluate(value, context, provider)?
                    .as_str()
                    .ok_or(EvalError::NotAString)?
                    .to_string();
                let p = Self::evaluate(pattern, context, provider)?
                    .as_str()
                    .ok_or(EvalError::NotAString)?
                    .to_string();
                let r = Self::evaluate(replacement, context, provider)?
                    .as_str()
                    .ok_or(EvalError::NotAString)?
                    .to_string();
                Ok(Cow::Owned(Value::String(v.replace(&p, &r))))
            }
            Expr::Concat(list) => {
                let mut res = String::new();
                for e in list {
                    let v = Self::evaluate(e, context, provider)?;
                    if let Some(s) = v.as_str() {
                        res.push_str(s);
                    } else {
                        res.push_str(&v.to_string());
                    }
                }
                Ok(Cow::Owned(Value::String(res)))
            }

            // --- Collections Standard ---
            Expr::Len(e) => {
                let v = Self::evaluate(e, context, provider)?;
                match v.as_ref() {
                    Value::Array(a) => Ok(Cow::Owned(json!(a.len()))),
                    Value::String(s) => Ok(Cow::Owned(json!(s.len()))),
                    _ => Ok(Cow::Owned(json!(0))),
                }
            }
            Expr::Contains { list, value } => {
                let col = Self::evaluate(list, context, provider)?;
                let target = Self::evaluate(value, context, provider)?;
                match col.as_ref() {
                    Value::Array(arr) => Ok(Cow::Owned(Value::Bool(arr.contains(&target)))),
                    Value::String(s) => {
                        let sub = target.as_str().unwrap_or("");
                        Ok(Cow::Owned(Value::Bool(s.contains(sub))))
                    }
                    _ => Ok(Cow::Owned(Value::Bool(false))),
                }
            }
            Expr::Min(e) => {
                let val = Self::evaluate(e, context, provider)?;
                let arr = val.as_array().ok_or(EvalError::NotAnArray)?;
                let min_val = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(f64::INFINITY, |a, b| a.min(b));
                if min_val == f64::INFINITY {
                    Ok(Cow::Owned(Value::Null))
                } else {
                    Ok(Cow::Owned(smart_number(min_val)))
                }
            }
            Expr::Max(e) => {
                let val = Self::evaluate(e, context, provider)?;
                let arr = val.as_array().ok_or(EvalError::NotAnArray)?;
                let max_val = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(f64::NEG_INFINITY, |a, b| a.max(b));
                if max_val == f64::NEG_INFINITY {
                    Ok(Cow::Owned(Value::Null))
                } else {
                    Ok(Cow::Owned(smart_number(max_val)))
                }
            }

            // --- Structure de Contrôle ---
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let val_cond = Self::evaluate(condition, context, provider)?;
                if is_truthy(&val_cond) {
                    Self::evaluate(then_branch, context, provider)
                } else {
                    Self::evaluate(else_branch, context, provider)
                }
            }

            // --- Dates ---
            Expr::Now => Ok(Cow::Owned(json!(Utc::now().to_rfc3339()))),
            Expr::DateAdd { date, days } => {
                let d_val = Self::evaluate(date, context, provider)?;
                let days_val = Self::evaluate(days, context, provider)?
                    .as_i64()
                    .unwrap_or(0);

                let d_str = d_val.as_str().ok_or(EvalError::NotAString)?;
                if let Ok(dt) = DateTime::parse_from_rfc3339(d_str) {
                    let new_dt = dt + Duration::days(days_val);
                    Ok(Cow::Owned(json!(new_dt.to_rfc3339())))
                } else if let Ok(nd) = NaiveDate::parse_from_str(d_str, "%Y-%m-%d") {
                    let new_nd = nd + Duration::days(days_val);
                    Ok(Cow::Owned(json!(new_nd.format("%Y-%m-%d").to_string())))
                } else {
                    Err(EvalError::InvalidDate(d_str.to_string()))
                }
            }
            Expr::DateDiff { start, end } => {
                let s_val = Self::evaluate(start, context, provider)?;
                let e_val = Self::evaluate(end, context, provider)?;

                let s_str = s_val.as_str().ok_or(EvalError::NotAString)?;
                let e_str = e_val.as_str().ok_or(EvalError::NotAString)?;

                if let (Ok(dt1), Ok(dt2)) = (
                    DateTime::parse_from_rfc3339(s_str),
                    DateTime::parse_from_rfc3339(e_str),
                ) {
                    let diff = dt2.signed_duration_since(dt1).num_days();
                    Ok(Cow::Owned(json!(diff)))
                } else if let (Ok(nd1), Ok(nd2)) = (
                    NaiveDate::parse_from_str(s_str, "%Y-%m-%d"),
                    NaiveDate::parse_from_str(e_str, "%Y-%m-%d"),
                ) {
                    let diff = nd2.signed_duration_since(nd1).num_days();
                    Ok(Cow::Owned(json!(diff)))
                } else {
                    Err(EvalError::InvalidDate(format!("{} ou {}", s_str, e_str)))
                }
            }

            // --- Lookup ---
            Expr::Lookup {
                collection,
                id,
                field,
            } => {
                let id_v = Self::evaluate(id, context, provider)?;
                let id_s = id_v.as_str().unwrap_or("");
                let res = provider
                    .get_value(collection, id_s, field)
                    .unwrap_or(Value::Null);
                Ok(Cow::Owned(res))
            }
        }
    }
}

// --- Helpers ---

// OPTIMISATION : Convertit les floats en int si pas de décimales (pour compatibilité as_i64)
fn smart_number(n: f64) -> Value {
    if n.fract() == 0.0 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

fn resolve_path<'a>(context: &'a Value, path: &str) -> Result<Cow<'a, Value>, EvalError> {
    let mut current = context;
    if path.is_empty() {
        return Ok(Cow::Borrowed(current));
    }

    for part in path.split('.') {
        match current {
            Value::Object(map) => {
                current = map
                    .get(part)
                    .ok_or_else(|| EvalError::VarNotFound(path.to_string()))?;
            }
            Value::Array(arr) => {
                let idx = part
                    .parse::<usize>()
                    .map_err(|_| EvalError::Generic("Index invalide".into()))?;
                current = arr
                    .get(idx)
                    .ok_or_else(|| EvalError::Generic("Index hors limites".into()))?;
            }
            _ => {
                return Err(EvalError::Generic(format!(
                    "Impossible d'accéder à {} sur une primitive",
                    part
                )))
            }
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

fn compare_nums<'a, F>(
    a: &Expr,
    b: &Expr,
    c: &'a Value,
    p: &dyn DataProvider,
    op: F,
) -> Result<Cow<'a, Value>, EvalError>
where
    F: Fn(f64, f64) -> bool,
{
    let va = Evaluator::evaluate(a, c, p)?
        .as_f64()
        .ok_or(EvalError::NotANumber)?;
    let vb = Evaluator::evaluate(b, c, p)?
        .as_f64()
        .ok_or(EvalError::NotANumber)?;
    Ok(Cow::Owned(Value::Bool(op(va, vb))))
}

fn fold_nums<'a, F>(
    list: &[Expr],
    c: &'a Value,
    p: &dyn DataProvider,
    init: f64,
    op: F,
) -> Result<Cow<'a, Value>, EvalError>
where
    F: Fn(f64, f64) -> f64,
{
    let mut acc = init;
    for e in list {
        let val = Evaluator::evaluate(e, c, p)?
            .as_f64()
            .ok_or(EvalError::NotANumber)?;
        acc = op(acc, val);
    }
    Ok(Cow::Owned(smart_number(acc)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_eq_vec_variant() {
        let provider = NoOpDataProvider;
        let ctx = json!({});

        let expr_true = Expr::Eq(vec![Expr::Val(json!(10)), Expr::Val(json!(10))]);
        assert_eq!(
            Evaluator::evaluate(&expr_true, &ctx, &provider)
                .unwrap()
                .as_bool(),
            Some(true)
        );
    }

    #[test]
    fn test_date_ops() {
        let provider = NoOpDataProvider;
        let ctx = json!({});
        let expr_add = Expr::DateAdd {
            date: Box::new(Expr::Val(json!("2023-01-01"))),
            days: Box::new(Expr::Val(json!(5))),
        };
        let res = Evaluator::evaluate(&expr_add, &ctx, &provider).unwrap();
        assert_eq!(res.as_str(), Some("2023-01-06"));
    }
}
