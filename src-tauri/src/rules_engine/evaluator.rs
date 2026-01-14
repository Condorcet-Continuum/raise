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
        expr: &Expr,
        context: &'a Value,
        provider: &dyn DataProvider,
    ) -> Result<Cow<'a, Value>, EvalError> {
        Self::eval_recursive(expr, context, &[], provider)
    }

    fn eval_recursive<'a>(
        expr: &Expr,
        root: &'a Value,
        scope: &[(&str, &'a Value)],
        provider: &dyn DataProvider,
    ) -> Result<Cow<'a, Value>, EvalError> {
        match expr {
            Expr::Val(v) => Ok(Cow::Owned(v.clone())),

            Expr::Var(path) => {
                for (alias, val) in scope.iter().rev() {
                    if path == *alias {
                        return Ok(Cow::Borrowed(*val));
                    }
                    if path.starts_with(&format!("{}.", alias)) {
                        let sub_path = &path[alias.len() + 1..];
                        let ptr = format!("/{}", sub_path.replace('.', "/"));
                        return val
                            .pointer(&ptr)
                            .map(Cow::Borrowed)
                            .ok_or_else(|| EvalError::VarNotFound(path.clone()));
                    }
                }
                let ptr = if path.starts_with('/') {
                    path.clone()
                } else {
                    format!("/{}", path.replace('.', "/"))
                };
                root.pointer(&ptr)
                    .map(Cow::Borrowed)
                    .ok_or_else(|| EvalError::VarNotFound(path.clone()))
            }

            Expr::Now => Ok(Cow::Owned(json!(Utc::now().to_rfc3339()))),

            // --- LISTES ---
            Expr::Len(arg) => {
                let val = Self::eval_recursive(arg, root, scope, provider)?;
                match &*val {
                    Value::Array(arr) => Ok(Cow::Owned(json!(arr.len()))),
                    Value::String(s) => Ok(Cow::Owned(json!(s.len()))),
                    _ => Err(EvalError::Generic(format!("Len() invalide sur {:?}", val))),
                }
            }
            Expr::Min(arg) | Expr::Max(arg) => {
                let val = Self::eval_recursive(arg, root, scope, provider)?;
                let arr = val.as_array().ok_or(EvalError::NotAnArray)?;
                if arr.is_empty() {
                    return Ok(Cow::Owned(Value::Null));
                }

                let mut iter = arr.iter().filter_map(|v| v.as_f64());
                let first = iter.next().ok_or(EvalError::NotANumber)?;

                let result = if matches!(expr, Expr::Min(_)) {
                    iter.fold(first, f64::min)
                } else {
                    iter.fold(first, f64::max)
                };
                Ok(Cow::Owned(json!(result)))
            }
            Expr::Contains { list, value } => {
                let l_val = Self::eval_recursive(list, root, scope, provider)?;
                let v_val = Self::eval_recursive(value, root, scope, provider)?;
                match &*l_val {
                    Value::Array(arr) => Ok(Cow::Owned(json!(arr.contains(&v_val)))),
                    Value::String(s) => match &*v_val {
                        Value::String(sub) => Ok(Cow::Owned(json!(s.contains(sub)))),
                        _ => Ok(Cow::Owned(json!(false))),
                    },
                    _ => Err(EvalError::NotAnArray),
                }
            }
            Expr::Map {
                list,
                alias,
                expr: map_expr,
            } => {
                let l_val = Self::eval_recursive(list, root, scope, provider)?;
                let arr = l_val.as_array().ok_or(EvalError::NotAnArray)?;
                let mut result = Vec::with_capacity(arr.len());
                for item in arr {
                    let mut new_scope = Vec::with_capacity(scope.len() + 1);
                    new_scope.extend_from_slice(scope);
                    new_scope.push((alias.as_str(), item));
                    let mapped = Self::eval_recursive(map_expr, root, &new_scope, provider)?;
                    result.push(mapped.into_owned());
                }
                Ok(Cow::Owned(Value::Array(result)))
            }
            Expr::Filter {
                list,
                alias,
                condition,
            } => {
                let l_val = Self::eval_recursive(list, root, scope, provider)?;
                let arr = l_val.as_array().ok_or(EvalError::NotAnArray)?;
                let mut result = Vec::new();
                for item in arr {
                    let mut new_scope = Vec::with_capacity(scope.len() + 1);
                    new_scope.extend_from_slice(scope);
                    new_scope.push((alias.as_str(), item));
                    let keep = Self::eval_recursive(condition, root, &new_scope, provider)?;
                    if is_truthy(&keep) {
                        result.push(item.clone());
                    }
                }
                Ok(Cow::Owned(Value::Array(result)))
            }

            // --- MATHS ---
            Expr::Add(args) => {
                let mut sum = 0.0;
                for arg in args {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    sum += as_f64(&val)?;
                }
                Ok(Cow::Owned(json!(sum)))
            }
            Expr::Sub(args) => {
                if args.is_empty() {
                    return Ok(Cow::Owned(json!(0.0)));
                }
                let mut iter = args.iter();
                let first = Self::eval_recursive(iter.next().unwrap(), root, scope, provider)?;
                let mut result = as_f64(&first)?;
                for arg in iter {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    result -= as_f64(&val)?;
                }
                Ok(Cow::Owned(json!(result)))
            }
            Expr::Mul(args) => {
                let mut prod = 1.0;
                for arg in args {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    prod *= as_f64(&val)?;
                }
                Ok(Cow::Owned(json!(prod)))
            }
            Expr::Div(args) => {
                if args.is_empty() {
                    return Ok(Cow::Owned(json!(1.0)));
                }
                let mut iter = args.iter();
                let first = Self::eval_recursive(iter.next().unwrap(), root, scope, provider)?;
                let mut result = as_f64(&first)?;
                for arg in iter {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    let divisor = as_f64(&val)?;
                    if divisor == 0.0 {
                        return Ok(Cow::Owned(Value::Null));
                    }
                    result /= divisor;
                }
                Ok(Cow::Owned(json!(result)))
            }
            Expr::Abs(arg) => {
                let val = Self::eval_recursive(arg, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_f64(&val)?.abs())))
            }
            Expr::Round { value, precision } => {
                let v_cow = Self::eval_recursive(value, root, scope, provider)?;
                let p_cow = Self::eval_recursive(precision, root, scope, provider)?;

                let v = as_f64(&v_cow)?;
                let p = as_f64(&p_cow)? as i32;

                let multiplier = 10f64.powi(p);
                let rounded = (v * multiplier).round() / multiplier;
                Ok(Cow::Owned(json!(rounded)))
            }

            // --- STRINGS ---
            Expr::Concat(args) => {
                let mut result = String::new();
                for arg in args {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    match &*val {
                        Value::String(s) => result.push_str(s),
                        Value::Number(n) => {
                            // CORRECTION ICI : "Smart Stringify"
                            // Si c'est un float qui ressemble à un entier (ex: 5000.0), on vire le .0
                            // Sinon on garde le formatage par défaut.
                            if let Some(f) = n.as_f64() {
                                if f.fract() == 0.0 {
                                    // C'est un entier stocké en float -> on affiche sans décimale
                                    result.push_str(&(f as i64).to_string());
                                } else {
                                    result.push_str(&n.to_string());
                                }
                            } else {
                                // Cas où c'est déjà un i64/u64
                                result.push_str(&n.to_string());
                            }
                        }
                        Value::Bool(b) => result.push_str(&b.to_string()),
                        _ => {}
                    }
                }
                Ok(Cow::Owned(json!(result)))
            }
            Expr::Upper(arg) => {
                let val = Self::eval_recursive(arg, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_string(&val)?.to_uppercase())))
            }
            Expr::Lower(arg) => {
                let val = Self::eval_recursive(arg, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_string(&val)?.to_lowercase())))
            }
            Expr::Trim(arg) => {
                let val = Self::eval_recursive(arg, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_string(&val)?.trim())))
            }
            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                let v = Self::eval_recursive(value, root, scope, provider)?;
                let p = Self::eval_recursive(pattern, root, scope, provider)?;
                let r = Self::eval_recursive(replacement, root, scope, provider)?;
                Ok(Cow::Owned(json!(
                    as_string(&v)?.replace(as_string(&p)?, as_string(&r)?)
                )))
            }
            Expr::RegexMatch { value, pattern } => {
                let val_res = Self::eval_recursive(value, root, scope, provider)?;
                let pat_res = Self::eval_recursive(pattern, root, scope, provider)?;
                let re = Regex::new(as_string(&pat_res)?)
                    .map_err(|e| EvalError::InvalidRegex(e.to_string()))?;
                Ok(Cow::Owned(json!(re.is_match(as_string(&val_res)?))))
            }

            // --- DATES ---
            Expr::DateDiff { start, end } => {
                let s_val = Self::eval_recursive(start, root, scope, provider)?;
                let e_val = Self::eval_recursive(end, root, scope, provider)?;
                let s_date = parse_date(as_string(&s_val)?)?;
                let e_date = parse_date(as_string(&e_val)?)?;
                Ok(Cow::Owned(json!(e_date
                    .signed_duration_since(s_date)
                    .num_days())))
            }
            Expr::DateAdd { date, days } => {
                let d_val = Self::eval_recursive(date, root, scope, provider)?;
                let days_val = Self::eval_recursive(days, root, scope, provider)?;
                let parsed = parse_date(as_string(&d_val)?)?;
                let new_date = parsed + Duration::days(as_f64(&days_val)? as i64);
                Ok(Cow::Owned(json!(new_date.to_rfc3339())))
            }

            // --- LOOKUP & LOGIC ---
            Expr::Lookup {
                collection,
                id,
                field,
            } => {
                let id_val = Self::eval_recursive(id, root, scope, provider)?;
                match provider.get_value(collection, as_string(&id_val)?, field) {
                    Some(v) => Ok(Cow::Owned(v)),
                    None => Ok(Cow::Owned(Value::Null)),
                }
            }
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = Self::eval_recursive(condition, root, scope, provider)?;
                if is_truthy(&cond) {
                    Self::eval_recursive(then_branch, root, scope, provider)
                } else {
                    Self::eval_recursive(else_branch, root, scope, provider)
                }
            }
            Expr::Eq(a, b) => {
                let va = Self::eval_recursive(a, root, scope, provider)?;
                let vb = Self::eval_recursive(b, root, scope, provider)?;
                Ok(Cow::Owned(json!(va.as_ref() == vb.as_ref())))
            }
            Expr::Neq(a, b) => {
                let va = Self::eval_recursive(a, root, scope, provider)?;
                let vb = Self::eval_recursive(b, root, scope, provider)?;
                Ok(Cow::Owned(json!(va.as_ref() != vb.as_ref())))
            }
            Expr::Gt(a, b) => {
                let va = Self::eval_recursive(a, root, scope, provider)?;
                let vb = Self::eval_recursive(b, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_f64(&va)? > as_f64(&vb)?)))
            }
            Expr::Lt(a, b) => {
                let va = Self::eval_recursive(a, root, scope, provider)?;
                let vb = Self::eval_recursive(b, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_f64(&va)? < as_f64(&vb)?)))
            }
            Expr::Gte(a, b) => {
                let va = Self::eval_recursive(a, root, scope, provider)?;
                let vb = Self::eval_recursive(b, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_f64(&va)? >= as_f64(&vb)?)))
            }
            Expr::Lte(a, b) => {
                let va = Self::eval_recursive(a, root, scope, provider)?;
                let vb = Self::eval_recursive(b, root, scope, provider)?;
                Ok(Cow::Owned(json!(as_f64(&va)? <= as_f64(&vb)?)))
            }
            Expr::And(args) => {
                for arg in args {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    if !is_truthy(&val) {
                        return Ok(Cow::Owned(json!(false)));
                    }
                }
                Ok(Cow::Owned(json!(true)))
            }
            Expr::Or(args) => {
                for arg in args {
                    let val = Self::eval_recursive(arg, root, scope, provider)?;
                    if is_truthy(&val) {
                        return Ok(Cow::Owned(json!(true)));
                    }
                }
                Ok(Cow::Owned(json!(false)))
            }
            Expr::Not(inner) => {
                let val = Self::eval_recursive(inner, root, scope, provider)?;
                Ok(Cow::Owned(json!(!is_truthy(&val))))
            }
        }
    }
}

// Helpers internes
fn as_string(v: &Value) -> Result<&str, EvalError> {
    v.as_str().ok_or(EvalError::NotAString)
}

fn as_f64(v: &Value) -> Result<f64, EvalError> {
    v.as_f64().ok_or(EvalError::NotANumber)
}

fn parse_date(s: &str) -> Result<DateTime<Utc>, EvalError> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return Ok(DateTime::<Utc>::from_naive_utc_and_offset(
            d.and_hms_opt(0, 0, 0).unwrap(),
            Utc,
        ));
    }
    Err(EvalError::InvalidDate(s.to_string()))
}

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;

    // ... les tests existants restent inchangés ...
    #[test]
    fn test_list_operations_map() {
        let ctx = json!({
            "items": [10, 20],
            "tax": 1.2
        });

        let expr = Expr::Map {
            list: Box::new(Expr::Var("items".into())),
            alias: "x".into(),
            expr: Box::new(Expr::Mul(vec![
                Expr::Var("x".into()),
                Expr::Var("tax".into()),
            ])),
        };

        let provider = NoOpDataProvider;
        let res = Evaluator::evaluate(&expr, &ctx, &provider).unwrap();

        let arr = res.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_f64(), Some(12.0));
        assert_eq!(arr[1].as_f64(), Some(24.0));
    }

    #[test]
    fn test_list_operations_filter() {
        let ctx = json!({ "ages": [10, 18, 25, 5] });

        let expr = Expr::Filter {
            list: Box::new(Expr::Var("ages".into())),
            alias: "a".into(),
            condition: Box::new(Expr::Gte(
                Box::new(Expr::Var("a".into())),
                Box::new(Expr::Val(json!(18))),
            )),
        };

        let provider = NoOpDataProvider;
        let res = Evaluator::evaluate(&expr, &ctx, &provider).unwrap();

        let arr = res.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0].as_i64(), Some(18));
        assert_eq!(arr[1].as_i64(), Some(25));
    }

    #[test]
    fn test_extended_stdlib() {
        let provider = NoOpDataProvider;
        let ctx = json!({
            "txt": "  Hello World  ",
            "nums": [10, 5, 20],
            "price": 10.556
        });

        // Trim
        let t = Expr::Trim(Box::new(Expr::Var("txt".into())));
        assert_eq!(
            Evaluator::evaluate(&t, &ctx, &provider)
                .unwrap()
                .into_owned(),
            json!("Hello World")
        );

        // Lower
        let l = Expr::Lower(Box::new(Expr::Var("txt".into())));
        assert_eq!(
            Evaluator::evaluate(&l, &ctx, &provider)
                .unwrap()
                .into_owned(),
            json!("  hello world  ")
        );

        // Round
        let r = Expr::Round {
            value: Box::new(Expr::Var("price".into())),
            precision: Box::new(Expr::Val(json!(2))),
        };
        assert_eq!(
            Evaluator::evaluate(&r, &ctx, &provider).unwrap().as_f64(),
            Some(10.56)
        );

        // Min/Max
        let min = Expr::Min(Box::new(Expr::Var("nums".into())));
        assert_eq!(
            Evaluator::evaluate(&min, &ctx, &provider).unwrap().as_f64(),
            Some(5.0)
        );

        let max = Expr::Max(Box::new(Expr::Var("nums".into())));
        assert_eq!(
            Evaluator::evaluate(&max, &ctx, &provider).unwrap().as_f64(),
            Some(20.0)
        );
    }
}
