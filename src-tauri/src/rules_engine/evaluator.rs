// FICHIER : src-tauri/src/rules_engine/evaluator.rs
use crate::rules_engine::ast::Expr;
use crate::utils::prelude::*;

// 🎯 MIGRATION V1.3 :
// L'énumération `EvalError` et son `impl From` ont été TOTALEMENT SUPPRIMÉS.
// Tout le fichier utilise dorénavant nativement `RaiseResult` et les macros du socle.

/// Trait permettant aux règles d'accéder à des données externes (Lookups)
#[async_interface]
pub trait DataProvider: Send + Sync {
    async fn get_value(&self, collection: &str, id: &str, field: &str) -> Option<JsonValue>;
}

pub struct NoOpDataProvider;
#[async_interface]
impl DataProvider for NoOpDataProvider {
    async fn get_value(&self, _c: &str, _id: &str, _f: &str) -> Option<JsonValue> {
        None
    }
}

pub struct Evaluator;

impl Evaluator {
    // 🎯 MIGRATION : Remplacement du Result standard par RaiseResult
    pub async fn evaluate<'a>(
        expr: &'a Expr,
        context: &'a JsonValue,
        provider: &dyn DataProvider,
    ) -> RaiseResult<CowData<'a, JsonValue>> {
        match expr {
            Expr::Val(v) => Ok(CowData::Borrowed(v)),
            Expr::Var(path) => resolve_path(context, path),

            // --- Opérateurs Logiques ---
            Expr::And(list) => {
                for e in list {
                    let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                    if !is_truthy(&val) {
                        return Ok(CowData::Owned(JsonValue::Bool(false)));
                    }
                }
                Ok(CowData::Owned(JsonValue::Bool(true)))
            }
            Expr::Or(list) => {
                for e in list {
                    let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                    if is_truthy(&val) {
                        return Ok(CowData::Owned(JsonValue::Bool(true)));
                    }
                }
                Ok(CowData::Owned(JsonValue::Bool(false)))
            }
            Expr::Not(e) => {
                let res = Box::pin(Self::evaluate(e, context, provider)).await?;
                Ok(CowData::Owned(JsonValue::Bool(!is_truthy(&res))))
            }

            // --- Comparaisons ---
            Expr::Eq(args) => {
                if args.len() < 2 {
                    return Ok(CowData::Owned(JsonValue::Bool(true)));
                }
                let first = Box::pin(Self::evaluate(&args[0], context, provider)).await?;
                for arg in &args[1..] {
                    let next = Box::pin(Self::evaluate(arg, context, provider)).await?;
                    if first != next {
                        return Ok(CowData::Owned(JsonValue::Bool(false)));
                    }
                }
                Ok(CowData::Owned(JsonValue::Bool(true)))
            }
            Expr::Neq(args) => {
                if args.len() < 2 {
                    return Ok(CowData::Owned(JsonValue::Bool(false)));
                }
                let a = Box::pin(Self::evaluate(&args[0], context, provider)).await?;
                let b = Box::pin(Self::evaluate(&args[1], context, provider)).await?;
                Ok(CowData::Owned(JsonValue::Bool(a != b)))
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
                    return Ok(CowData::Owned(json_value!(0)));
                }
                let first_val = Box::pin(Self::evaluate(&list[0], context, provider)).await?;

                // Initialisation sécurisée et typée de l'accumulateur
                let mut acc: f64 = match first_val.as_f64() {
                    Some(num) => num,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "aggregation_init",
                            "expected": "number (f64)",
                            "received": first_val,
                            "item_index": 0,
                            "hint": "Le premier élément de la liste doit être un nombre pour initialiser l'opération."
                        })
                    ),
                };
                for (index, e) in list[1..].iter().enumerate() {
                    let current_val = Box::pin(Self::evaluate(e, context, provider)).await?;

                    // Extraction numérique avec garde-fou
                    let val: f64 = match current_val.as_f64() {
                        Some(num) => num,
                        None => raise_error!(
                            "ERR_RULE_TYPE_MISMATCH",
                            context = json_value!({
                                "operation": "subtraction_loop",
                                "expected": "number (f64)",
                                "received": current_val,
                                "item_index": index + 1, // On ajuste l'index car on a skip le premier
                                "hint": "Chaque élément de la liste de soustraction doit être un nombre."
                            })
                        ),
                    };

                    acc -= val;
                }
                Ok(CowData::Owned(smart_number(acc)))
            }
            Expr::Div(list) => {
                if list.len() < 2 {
                    raise_error!(
                        "ERR_RULE_INVALID_ARGS",
                        error = "L'opérateur Div requiert au moins 2 arguments"
                    );
                }
                let first_val = Box::pin(Self::evaluate(&list[0], context, provider)).await?;
                let mut acc: f64 = match first_val.as_f64() {
                    Some(n) => n,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        error = "Le numérateur initial doit être un nombre"
                    ),
                };

                for e in list[1..].iter() {
                    let current_val = Box::pin(Self::evaluate(e, context, provider)).await?;
                    let val: f64 = match current_val.as_f64() {
                        Some(n) => n,
                        None => raise_error!(
                            "ERR_RULE_TYPE_MISMATCH",
                            error = "Les dénominateurs doivent être des nombres"
                        ),
                    };
                    if val == 0.0 {
                        raise_error!(
                            "ERR_RULE_DIV_BY_ZERO",
                            error = "Division par zéro interdite"
                        );
                    }
                    acc /= val;
                }
                Ok(CowData::Owned(smart_number(acc)))
            }
            Expr::Abs(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;

                // Extraction numérique stricte
                let v: f64 = match val.as_f64() {
                    Some(num) => num,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "ABS",
                            "expected": "number (f64)",
                            "received": val,
                            "hint": "La fonction ABS (valeur absolue) nécessite une valeur numérique en entrée."
                        })
                    ),
                };

                Ok(CowData::Owned(smart_number(v.abs())))
            }
            Expr::Round { value, precision } => {
                // 1. Évaluation de la valeur principale
                let val_res = Box::pin(Self::evaluate(value, context, provider)).await?;
                let v: f64 = match val_res.as_f64() {
                    Some(n) => n,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "ROUND",
                            "field": "value",
                            "expected": "number",
                            "received": val_res,
                            "hint": "La valeur à arrondir doit être un nombre."
                        })
                    ),
                };

                // 2. Évaluation de la précision (on garde le défaut à 0, mais on valide le type si présent)
                let prec_res = Box::pin(Self::evaluate(precision, context, provider)).await?;
                let p: i32 = match prec_res.as_i64() {
                    Some(n) => n as i32,
                    None => 0, // Valeur par défaut si non spécifié ou type invalide
                };

                // 3. Calcul mathématique
                let factor = 10f64.powi(p);
                let res = (v * factor).round() / factor;

                Ok(CowData::Owned(smart_number(res)))
            }

            Expr::Min(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let arr: &Vec<JsonValue> = match val.as_array() {
                    Some(array) => array,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "target": "rule_evaluation_result",
                            "expected": "array",
                            "received": val,
                            "action": "evaluate_array_rule",
                            "hint": "Le résultat de l'expression évaluée doit être un tableau pour cette règle."
                        })
                    ),
                };
                let min = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(f64::INFINITY, |a, b| a.min(b));

                if min.is_infinite() {
                    Ok(CowData::Owned(JsonValue::Null))
                } else {
                    Ok(CowData::Owned(smart_number(min)))
                }
            }
            Expr::Max(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;
                let arr: &Vec<JsonValue> = match val.as_array() {
                    Some(array) => array,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "MAX",
                            "expected": "array",
                            "received": val,
                            "hint": "L'opération MAX nécessite un tableau de nombres en entrée."
                        })
                    ),
                };

                let max = arr
                    .iter()
                    .filter_map(|v| v.as_f64())
                    .fold(f64::NEG_INFINITY, |a, b| a.max(b));

                if max.is_infinite() {
                    Ok(CowData::Owned(JsonValue::Null))
                } else {
                    Ok(CowData::Owned(smart_number(max)))
                }
            }

            Expr::Contains { list, value } => {
                let list_val = Box::pin(Self::evaluate(list, context, provider)).await?;
                let search_val = Box::pin(Self::evaluate(value, context, provider)).await?;

                let found = match list_val.as_array() {
                    Some(arr) => arr.contains(&*search_val),
                    None => match list_val.as_str() {
                        Some(s) => {
                            let search_str = match search_val.as_str() {
                                Some(ss) => ss,
                                None => raise_error!(
                                    "ERR_RULE_TYPE_MISMATCH",
                                    context = json_value!({"expected": "string", "hint": "Recherche dans une chaîne nécessite une chaîne."})
                                ),
                            };
                            s.contains(search_str)
                        }
                        None => raise_error!(
                            "ERR_RULE_TYPE_MISMATCH",
                            context = json_value!({ "operation": "CONTAINS", "expected": ["array", "string"], "received": list_val })
                        ),
                    },
                };
                Ok(CowData::Owned(JsonValue::Bool(found)))
            }

            // --- Collections & Itérations ---
            Expr::Map {
                list,
                alias,
                expr: map_expr,
            } => {
                let list_val = Box::pin(Self::evaluate(list, context, provider)).await?;
                let arr: &Vec<JsonValue> = match list_val.as_array() {
                    Some(array) => array,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "target": "list_operation",
                            "expected": "array",
                            "received": list_val,
                            "action": "process_collection",
                            "hint": "L'opération attend un tableau de données. Vérifiez que la propriété ciblée n'est pas nulle ou d'un autre type."
                        })
                    ),
                };

                let mut result_arr = Vec::new();
                for item in arr {
                    let mut local_ctx = context.clone();
                    if let Some(obj) = local_ctx.as_object_mut() {
                        obj.insert(alias.clone(), item.clone());
                    }
                    let res = Box::pin(Self::evaluate(map_expr, &local_ctx, provider)).await?;
                    result_arr.push(res.into_owned());
                }
                Ok(CowData::Owned(JsonValue::Array(result_arr)))
            }
            Expr::Filter {
                list,
                alias,
                condition,
            } => {
                let list_val = Box::pin(Self::evaluate(list, context, provider)).await?;
                // Extraction sécurisée avec annotation de type pour stabiliser l'inférence
                let arr: &Vec<JsonValue> = match list_val.as_array() {
                    Some(array) => array,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "target": "list_operation",
                            "expected": "array",
                            "received": list_val,
                            "action": "process_collection",
                            "hint": "L'opération attend un tableau de données. Vérifiez que la propriété ciblée n'est pas nulle ou d'un autre type."
                        })
                    ),
                };
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
                Ok(CowData::Owned(JsonValue::Array(result_arr)))
            }

            // --- String & Regex ---
            Expr::RegexMatch { value, pattern } => {
                let v_str = Box::pin(Self::evaluate(value, context, provider)).await?;
                let p_str = Box::pin(Self::evaluate(pattern, context, provider)).await?;
                let v = match v_str.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "target": "validation_value",
                            "expected": "string",
                            "received": v_str,
                            "action": "extract_value_for_regex",
                            "hint": "La valeur à comparer doit être une chaîne de caractères pour être traitée par une Regex."
                        })
                    ),
                };
                let p = match p_str.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "expected": "string",
                            "received": p_str,
                            "action": "parse_rule_pattern",
                            "hint": "La règle attend une chaîne de caractères (Regex). Vérifiez que la valeur n'est pas un nombre ou un booléen dans votre fichier JSON."
                        })
                    ),
                };

                let re = match TextRegex::new(p) {
                    Ok(r) => r,
                    Err(e) => raise_error!(
                        "ERR_RULE_INVALID_REGEX",
                        error = e,
                        context = json_value!({
                            "pattern": p,
                            "action": "compile_validation_rule",
                            "hint": "La syntaxe de l'expression régulière est invalide. Vérifiez les caractères d'échappement et les groupes."
                        })
                    ),
                };
                Ok(CowData::Owned(JsonValue::Bool(re.is_match(v))))
            }
            Expr::Concat(list) => {
                let mut res = String::new();
                for e in list {
                    let v = Box::pin(Self::evaluate(e, context, provider)).await?;
                    res.push_str(v.as_str().unwrap_or(&v.to_string()));
                }
                Ok(CowData::Owned(JsonValue::String(res)))
            }

            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                let v_val = Box::pin(Self::evaluate(value, context, provider)).await?;
                let p_val = Box::pin(Self::evaluate(pattern, context, provider)).await?;
                let r_val = Box::pin(Self::evaluate(replacement, context, provider)).await?;

                // 1. Extraction de la valeur (v)
                let v = match v_val.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({ "target": "v_val", "expected": "string", "received": v_val })
                    ),
                };

                // 2. Extraction du pattern (p)
                let p = match p_val.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({ "target": "p_val", "expected": "string", "received": p_val })
                    ),
                };

                // 3. Extraction du remplacement ou résultat (r)
                let r = match r_val.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({ "target": "r_val", "expected": "string", "received": r_val })
                    ),
                };

                Ok(CowData::Owned(JsonValue::String(v.replace(p, r))))
            }

            Expr::Upper(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;

                // Extraction sécurisée du texte à transformer
                let s = match val.as_str() {
                    Some(string_value) => string_value,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "UPPER",
                            "expected": "string",
                            "received": val,
                            "hint": "La fonction UPPER ne peut transformer que des chaînes de caractères."
                        })
                    ),
                };

                Ok(CowData::Owned(JsonValue::String(s.to_uppercase())))
            }
            Expr::Lower(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;

                // Extraction impérative pour stabiliser le type 's'
                let s = match val.as_str() {
                    Some(string_value) => string_value,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "LOWER",
                            "expected": "string",
                            "received": val,
                            "hint": "La fonction LOWER nécessite une chaîne de caractères en entrée."
                        })
                    ),
                };

                Ok(CowData::Owned(JsonValue::String(s.to_lowercase())))
            }
            Expr::Trim(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;

                // Extraction impérative pour un typage fort
                let s = match val.as_str() {
                    Some(string_value) => string_value,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "TRIM",
                            "expected": "string",
                            "received": val,
                            "hint": "La fonction TRIM ne peut traiter que des chaînes de caractères (suppression des espaces)."
                        })
                    ),
                };

                Ok(CowData::Owned(JsonValue::String(s.trim().to_string())))
            }

            Expr::Len(e) => {
                let val = Box::pin(Self::evaluate(e, context, provider)).await?;

                // Calcul de la longueur avec validation de type stricte
                let len = match val.as_ref() {
                    JsonValue::Array(arr) => arr.len(),
                    JsonValue::String(s) => s.chars().count(), // Gestion correcte de l'Unicode
                    JsonValue::Object(obj) => obj.len(),
                    _ => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "operation": "LEN",
                            "expected": ["array", "string", "object"],
                            "received": val,
                            "hint": "La fonction LEN ne peut être calculée que sur des listes, des chaînes ou des objets."
                        })
                    ),
                };

                Ok(CowData::Owned(json_value!(len)))
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
            Expr::Now => Ok(CowData::Owned(json_value!(UtcClock::now().to_rfc3339()))),
            Expr::DateAdd { date, days } => {
                let d_val = Box::pin(Self::evaluate(date, context, provider)).await?;
                let days_res = Box::pin(Self::evaluate(days, context, provider)).await?;
                let days_val = match days_res.as_i64() {
                    Some(d) => d,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({ "expected": "integer", "received": days_res, "hint": "DateAdd nécessite un nombre de jours entier." })
                    ),
                };
                let d_str = match d_val.as_str() {
                    Some(s) => s,
                    None => raise_error!(
                        "ERR_RULE_TYPE_MISMATCH",
                        context = json_value!({
                            "target": "d_val",
                            "expected": "string",
                            "received": d_val,
                            "action": "evaluate_expression_result",
                            "hint": "La valeur évaluée pour ce paramètre doit être une chaîne de caractères."
                        })
                    ),
                };

                if let Ok(local_dt) = d_str.parse::<LocalTimestamp>() {
                    Ok(CowData::Owned(json_value!((local_dt
                        + CalendarDuration::days(days_val))
                    .to_rfc3339())))

                // 🎯 3. CalendarDate fait déjà partie de ta façade !
                } else if let Ok(nd) = CalendarDate::parse_from_str(d_str, "%Y-%m-%d") {
                    Ok(CowData::Owned(json_value!((nd
                        + CalendarDuration::days(days_val))
                    .format("%Y-%m-%d")
                    .to_string())))
                } else {
                    crate::raise_error!(
                        "ERR_RULE_INVALID_DATE",
                        error = format!("Format de date invalide : {}", d_str)
                    );
                }
            }
            Expr::DateDiff { start: _, end: _ } => {
                raise_error!(
                    "ERR_NOT_IMPLEMENTED",
                    error = "DateDiff n'est pas encore implémenté"
                );
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
                    .unwrap_or(JsonValue::Null);
                Ok(CowData::Owned(res))
            }
        }
    }
}

// --- Helpers ---

// 🎯 MIGRATION : Remplacement du Result par RaiseResult
async fn compare_nums<'a, F>(
    a: &Expr,
    b: &Expr,
    c: &'a JsonValue,
    p: &dyn DataProvider,
    op: F,
) -> RaiseResult<CowData<'a, JsonValue>>
where
    F: Fn(f64, f64) -> bool,
{
    let val_a = Box::pin(Evaluator::evaluate(a, c, p)).await?;

    // Extraction impérative avec typage explicite pour stabiliser l'inférence
    let va: f64 = match val_a.as_f64() {
        Some(num) => num,
        None => raise_error!(
            "ERR_RULE_TYPE_MISMATCH",
            context = json_value!({
                "expected": "number (f64)",
                "received": val_a,
                "action": "numeric_comparison",
                "hint": "L'opération nécessite une valeur numérique. Vérifiez que l'expression n'évalue pas à une chaîne ou un objet."
            })
        ),
    };
    let val_b = Box::pin(Evaluator::evaluate(b, c, p)).await?;

    // Extraction impérative pour vb
    let vb: f64 = match val_b.as_f64() {
        Some(num) => num,
        None => raise_error!(
            "ERR_RULE_TYPE_MISMATCH",
            context = json_value!({
                "expected": "number (f64)",
                "side": "right-hand / operand B",
                "received": val_b,
                "action": "numeric_comparison",
                "hint": "Le deuxième membre de la comparaison n'est pas un nombre valide."
            })
        ),
    };
    Ok(CowData::Owned(JsonValue::Bool(op(va, vb))))
}

async fn fold_nums<'a, F>(
    list: &[Expr],
    c: &'a JsonValue,
    p: &dyn DataProvider,
    init: f64,
    op: F,
) -> RaiseResult<CowData<'a, JsonValue>>
where
    F: Fn(f64, f64) -> f64,
{
    let mut acc = init;
    for (index, e) in list.iter().enumerate() {
        let current_val = Box::pin(Evaluator::evaluate(e, c, p)).await?;

        // Extraction numérique impérative
        let val: f64 = match current_val.as_f64() {
            Some(num) => num,
            None => raise_error!(
                "ERR_RULE_TYPE_MISMATCH",
                context = json_value!({
                    "operation": "aggregation",
                    "expected": "number (f64)",
                    "received": current_val,
                    "item_index": index,
                    "hint": "Tous les éléments de la liste doivent être des nombres pour cette opération mathématique."
                })
            ),
        };

        acc = op(acc, val);
    }
    Ok(CowData::Owned(smart_number(acc)))
}

fn smart_number(n: f64) -> JsonValue {
    if n.fract() == 0.0 {
        json_value!(n as i64)
    } else {
        json_value!(n)
    }
}

fn resolve_path<'a>(context: &'a JsonValue, path: &str) -> RaiseResult<CowData<'a, JsonValue>> {
    let mut current = context;
    if path.is_empty() {
        return Ok(CowData::Borrowed(current));
    }
    for part in path.split('.') {
        current = match current {
            JsonValue::Object(map) => match map.get(part) {
                Some(val) => val,
                None => raise_error!(
                    "ERR_RULE_VAR_NOT_FOUND",
                    context = json_value!({
                        "path": path,
                        "missing_part": part,
                        "action": "resolve_json_path",
                        "hint": format!("Le champ '{}' est introuvable dans l'objet actuel.", part)
                    })
                ),
            },
            _ => raise_error!(
                "ERR_RULE_PATH_RESOLUTION_FAIL",
                context = json_value!({
                    "path": path,
                    "failed_at": part,
                    "reason": "La valeur parente n'est pas un objet (map).",
                    "current_value": current
                })
            ),
        };
    }
    Ok(CowData::Borrowed(current))
}

fn is_truthy(v: &JsonValue) -> bool {
    match v {
        JsonValue::Bool(b) => *b,
        JsonValue::Null => false,
        JsonValue::Number(n) => n.as_f64().unwrap_or(0.0) != 0.0,
        JsonValue::String(s) => !s.is_empty(),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[async_test]
    async fn test_eq_async() -> RaiseResult<()> {
        let provider = NoOpDataProvider;
        let ctx = json_value!({});
        let expr = Expr::Eq(vec![Expr::Val(json_value!(10)), Expr::Val(json_value!(10))]);

        let res = match Evaluator::evaluate(&expr, &ctx, &provider).await {
            Ok(val) => val,
            Err(e) => raise_error!(
                "ERR_TEST_EVALUATION_FAILED",
                error = e.to_string(),
                context = json_value!({ "test": "test_eq_async" })
            ),
        };

        assert_eq!(res.as_bool(), Some(true));
        Ok(())
    }

    #[async_test]
    async fn test_lookup_mock() -> RaiseResult<()> {
        struct MockProvider;
        #[async_interface]
        impl DataProvider for MockProvider {
            async fn get_value(&self, _c: &str, _id: &str, _f: &str) -> Option<JsonValue> {
                Some(json_value!("Alice"))
            }
        }

        let expr = Expr::Lookup {
            collection: "users".into(),
            id: Box::new(Expr::Val(json_value!("u1"))),
            field: "name".into(),
        };
        let context_data = json_value!({});

        let res = match Evaluator::evaluate(&expr, &context_data, &MockProvider).await {
            Ok(val) => val,
            Err(e) => raise_error!(
                "ERR_TEST_EVALUATION_FAILED",
                error = e.to_string(),
                context = json_value!({ "test": "test_lookup_mock" })
            ),
        };

        assert_eq!(res.as_str(), Some("Alice"));
        Ok(())
    }
}
