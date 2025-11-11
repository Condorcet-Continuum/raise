use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::{json, Map, Value};
use uuid::Uuid;

use std::cell::Cell;

thread_local! {
    static MAX_PASSES: Cell<usize> = Cell::new(4);
    static STRICT_PTR: Cell<bool> = Cell::new(false);
}

/// Options d'exécution pour x_compute
#[derive(Clone, Copy, Debug)]
pub struct ComputeOptions {
    pub max_passes: usize,
    pub strict_ptr: bool,
}

#[derive(Copy, Clone)]
enum Scope {
    Root,
    Self_,
}
#[derive(Copy, Clone)]
enum Update {
    Always,
    IfMissing,
    IfNull,
}

fn parse_scope(s: Option<&str>) -> Scope {
    match s {
        Some("root") => Scope::Root,
        _ => Scope::Self_,
    }
}
fn parse_update(s: Option<&str>) -> Update {
    match s {
        Some("always") => Update::Always,
        Some("if_null") => Update::IfNull,
        _ => Update::IfMissing, // défaut
    }
}

/* =========================================================================================
 *  Helpers de détection (x_compute / default-like)
 * =======================================================================================*/
fn schema_has_x_compute(s: &Value) -> bool {
    if s.get("x_compute").is_some() {
        return true;
    }
    for key in ["allOf", "oneOf", "anyOf"] {
        if let Some(arr) = s.get(key).and_then(|v| v.as_array()) {
            if arr.iter().any(schema_has_x_compute) {
                return true;
            }
        }
    }
    false
}
fn schema_has_default_like(s: &Value) -> bool {
    if s.get("default").is_some() || s.get("const").is_some() || s.get("enum").is_some() {
        return true;
    }
    for key in ["allOf", "oneOf", "anyOf"] {
        if let Some(arr) = s.get(key).and_then(|v| v.as_array()) {
            if arr.iter().any(schema_has_default_like) {
                return true;
            }
        }
    }
    false
}

/* =========================================================================================
 *  Pointeurs JSON (#/...) avec scope root/self, ../ et fallback
 * =======================================================================================*/

/// Résout un pointeur "#/…".
/// - scope=Root : on part de la racine
/// - scope=Self : on part de l’objet courant (`base_path`) **puis**
///   *si le pointeur ne commence pas par "../" et qu’on ne trouve rien*, on retente à la racine.
///   (fallback désactivable via STRICT_PTR)
fn resolve_ptr<'a>(
    root: &'a Value,
    base_path: &[String],
    ptr: &str,
    scope: Scope,
) -> Option<&'a Value> {
    if !ptr.starts_with("#/") {
        return None;
    }

    let segs_full: Vec<&str> = ptr.trim_start_matches("#/").split('/').collect();

    // Nombre de "../" en tête
    let mut up = 0usize;
    while up < segs_full.len() && segs_full[up] == ".." {
        up += 1;
    }
    let tail = &segs_full[up..];

    // helper: parcourt root selon un chemin
    let traverse = |mut cur: &'a Value, path: &[&str]| -> Option<&'a Value> {
        for s in path {
            match cur {
                Value::Object(m) => {
                    cur = m.get(*s)?;
                }
                Value::Array(a) => {
                    cur = a.get(s.parse::<usize>().ok()?)?;
                }
                _ => return None,
            }
        }
        Some(cur)
    };

    // 1) tentative "self"
    if matches!(scope, Scope::Self_) {
        // point de départ = base_path
        let mut path: Vec<&str> = base_path.iter().map(|s| s.as_str()).collect();
        // appliquer les remontées
        for _ in 0..up {
            if !path.is_empty() {
                path.pop();
            }
        }
        // ajouter la suite
        path.extend(tail);

        if let Some(v) = traverse(root, &path) {
            return Some(v);
        }

        // 2) fallback racine seulement s’il n’y avait pas de "../"
        if up == 0 && !STRICT_PTR.with(|c| c.get()) {
            if let Some(v) = traverse(root, tail) {
                return Some(v);
            }
        }
        return None;
    }

    // scope root: simple
    traverse(root, tail)
}

/* =========================================================================================
 *  Helpers bool/nombre
 * =======================================================================================*/

fn is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::Null => false,
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                i != 0
            } else if let Some(f) = n.as_f64() {
                f != 0.0
            } else {
                true
            }
        }
        Value::String(s) => !s.is_empty() && s != "false" && s != "0",
        Value::Array(a) => !a.is_empty(),
        Value::Object(m) => !m.is_empty(),
    }
}

/// Convertit souplement en f64 (Number→f64, String→parse, Bool→1/0, Null→0)
fn to_f64_lossy(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse::<f64>().ok(),
        Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
        Value::Null => Some(0.0),
        _ => None,
    }
}

/* =========================================================================================
 *  Évaluateur de plan (plan/v1)
 * =======================================================================================*/
// Small helper: traverse a JSON Pointer relative to a base value
fn traverse_ptr<'a>(base: &'a Value, ptr: &str) -> Option<&'a Value> {
    if !ptr.starts_with("#/") {
        return None;
    }
    let mut cur = base;
    for seg in ptr.trim_start_matches("#/").split('/') {
        let seg = seg.replace("~1", "/").replace("~0", "~"); // unescape
        if let Ok(idx) = seg.parse::<usize>() {
            cur = cur.get(idx)?;
        } else {
            cur = cur.get(&seg)?;
        }
    }
    Some(cur)
}

fn eval_bool(
    root_snapshot: &Value,
    self_path: &[String],
    pred: &Value,
    scope: Scope,
) -> Result<bool> {
    match pred {
        Value::Bool(b) => Ok(*b),
        Value::Object(obj) => {
            if let Some(op) = obj.get("op").and_then(|v| v.as_str()) {
                match op {
                    "and" | "or" | "not" => {
                        let args = obj
                            .get("args")
                            .and_then(|v| v.as_array())
                            .ok_or_else(|| anyhow!("{} needs args[]", op))?;
                        let b = match op {
                            "and" => {
                                let mut acc = true;
                                for x in args {
                                    if !acc {
                                        break;
                                    }
                                    acc = eval_bool(root_snapshot, self_path, x, scope)?;
                                }
                                acc
                            }
                            "or" => {
                                let mut any = false;
                                for x in args {
                                    if eval_bool(root_snapshot, self_path, x, scope)? {
                                        any = true;
                                        break;
                                    }
                                }
                                any
                            }
                            "not" => {
                                let one = args.get(0).ok_or_else(|| anyhow!("not needs 1 arg"))?;
                                !eval_bool(root_snapshot, self_path, one, scope)?
                            }
                            _ => unreachable!(),
                        };
                        Ok(b)
                    }
                    "lt" | "le" | "gt" | "ge" | "eq" | "ne" => {
                        let args = obj
                            .get("args")
                            .and_then(|v| v.as_array())
                            .ok_or_else(|| anyhow!("{} needs args[]", op))?;
                        if args.len() != 2 {
                            return Err(anyhow!("{} needs exactly 2 args", op));
                        }
                        let a = eval_plan(root_snapshot, self_path, &args[0], scope)?;
                        let b = eval_plan(root_snapshot, self_path, &args[1], scope)?;
                        let out = match op {
                            "eq" => a == b,
                            "ne" => a != b,
                            _ => {
                                if let (Some(x), Some(y)) = (to_f64_lossy(&a), to_f64_lossy(&b)) {
                                    match op {
                                        "lt" => x < y,
                                        "le" => x <= y,
                                        "gt" => x > y,
                                        "ge" => x >= y,
                                        _ => unreachable!(),
                                    }
                                } else {
                                    false
                                }
                            }
                        };
                        Ok(out)
                    }
                    _ => {
                        // Autre op → on évalue et on applique la truthiness
                        let v = eval_plan(root_snapshot, self_path, pred, scope)?;
                        Ok(is_truthy(&v))
                    }
                }
            } else if obj.get("ptr").is_some() || obj.get("cond").is_some() {
                let v = eval_plan(root_snapshot, self_path, pred, scope)?;
                Ok(is_truthy(&v))
            } else {
                Ok(!obj.is_empty())
            }
        }
        _ => Ok(is_truthy(pred)),
    }
}

fn eval_plan(
    root_snapshot: &Value,
    self_path: &[String],
    plan: &Value,
    scope: Scope,
) -> Result<Value> {
    if let Some(obj) = plan.as_object() {
        // ptr (tolérant : Null si absent du snapshot)
        if let Some(ptr) = obj.get("ptr").and_then(|v| v.as_str()) {
            if let Some(v) = resolve_ptr(root_snapshot, self_path, ptr, scope) {
                return Ok(v.clone());
            } else {
                return Ok(Value::Null);
            }
        }

        // op
        if let Some(op) = obj.get("op").and_then(|v| v.as_str()) {
            return match op {
                // Générateurs
                "uuid_v4" => Ok(json!(Uuid::new_v4().to_string())),
                "now_rfc3339" => Ok(json!(Utc::now().to_rfc3339())),
                "now_ts_ms" => Ok(json!(Utc::now().timestamp_millis())), // i64 → OK

                // Arithmétique (report si un arg vaut Null)
                "add" | "sub" | "mul" | "div" => {
                    let args = obj
                        .get("args")
                        .and_then(|v| v.as_array())
                        .ok_or_else(|| anyhow!("{} needs args[]", op))?;

                    let mut vals: Vec<Value> = Vec::with_capacity(args.len());
                    for a in args {
                        let v = if a.is_object() {
                            eval_plan(root_snapshot, self_path, a, scope)?
                        } else {
                            a.clone()
                        };
                        if v.is_null() {
                            return Ok(Value::Null);
                        }
                        vals.push(v);
                    }

                    let mut nums = Vec::<f64>::with_capacity(vals.len());
                    for v in vals {
                        nums.push(
                            v.as_f64()
                                .ok_or_else(|| anyhow!("{} arg must be number", op))?,
                        );
                    }

                    let res = match op {
                        "add" => nums.iter().copied().sum(),
                        "sub" => nums.into_iter().reduce(|a, b| a - b).unwrap_or(0.0),
                        "mul" => nums.into_iter().fold(1.0, |acc, x| acc * x),
                        "div" => nums.into_iter().reduce(|a, b| a / b).unwrap_or(0.0),
                        _ => unreachable!(),
                    };
                    Ok(json!(res))
                }

                // Arrondi
                "round" => {
                    let v0 = obj
                        .get("args")
                        .and_then(|v| v.as_array())
                        .and_then(|a| a.get(0))
                        .ok_or_else(|| anyhow!("round needs args[0]"))?;
                    let v = eval_plan(root_snapshot, self_path, v0, scope)?;
                    if v.is_null() {
                        return Ok(Value::Null);
                    }
                    let x = v
                        .as_f64()
                        .ok_or_else(|| anyhow!("round arg must be number"))?;
                    let scale = obj.get("scale").and_then(|v| v.as_u64()).unwrap_or(0);
                    let p = 10_f64.powi(scale as i32);
                    Ok(json!((x * p).round() / p))
                }

                // Somme d’un champ dans un tableau
                "sum" => {
                    // Config
                    let from = obj
                        .get("from")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| anyhow!("sum.from (JSON Pointer) is required"))?;
                    let path_field = obj.get("path"); // may be a key ("n") or a JSON Pointer ("#/a/b")
                    let pred = obj.get("where"); // optional filter object

                    // Resolve array
                    let arr = resolve_ptr(root_snapshot, self_path, from, scope)
                        .and_then(|v| v.as_array())
                        .ok_or_else(|| anyhow!("sum.from must point to an array"))?;

                    // Simple predicate evaluator for `where` inside sum (element-relative)
                    let elem_matches = |el: &Value| -> Result<bool> {
                        if let Some(p) = pred {
                            if let Some(map) = p.as_object() {
                                let ptr = map
                                    .get("ptr")
                                    .and_then(|v| v.as_str())
                                    .ok_or_else(|| anyhow!("where.ptr is required"))?;
                                let op = map.get("op").and_then(|v| v.as_str()).unwrap_or("==");
                                let rhs = map.get("value").unwrap_or(&Value::Null);

                                let lhs_v = if ptr.starts_with("#/") {
                                    traverse_ptr(el, ptr).cloned().unwrap_or(Value::Null)
                                } else {
                                    el.get(ptr).cloned().unwrap_or(Value::Null)
                                };

                                // numeric compare if possible, else string, else equality
                                let res = match (lhs_v.as_f64(), rhs.as_f64()) {
                                    (Some(a), Some(b)) => match op {
                                        ">" => a > b,
                                        ">=" => a >= b,
                                        "<" => a < b,
                                        "<=" => a <= b,
                                        "!=" => a != b,
                                        _ => a == b,
                                    },
                                    _ => {
                                        let ls = match &lhs_v {
                                            Value::String(s) => s.clone(),
                                            _ => lhs_v.to_string(),
                                        };
                                        let rs = match rhs {
                                            Value::String(s) => s.clone(),
                                            _ => rhs.to_string(),
                                        };
                                        match op {
                                            "!=" => ls != rs,
                                            _ => ls == rs,
                                        }
                                    }
                                };
                                return Ok(res);
                            }
                        }
                        Ok(true)
                    };

                    let mut total: f64 = 0.0;
                    for el in arr {
                        if !elem_matches(el)? {
                            continue;
                        }
                        // Extract the value to sum
                        let v_opt = match path_field {
                            Some(Value::String(s)) if s.starts_with("#/") => traverse_ptr(el, s),
                            Some(Value::String(s)) => el.get(s),
                            Some(_) => None,
                            None => Some(el),
                        };
                        if let Some(n) = v_opt.and_then(|v| v.as_f64()) {
                            total += n;
                        }
                    }

                    // Optional rounding scale
                    if let Some(scale) = obj.get("scale").and_then(|v| v.as_u64()) {
                        let p = 10f64.powi(scale as i32);
                        total = (total * p).round() / p;
                    }
                    Ok(json!(total))
                }

                // Conditionnel
                "cond" => {
                    let pred = obj
                        .get("if")
                        .ok_or_else(|| anyhow!("cond/if needs field 'if'"))?;
                    let then_p = obj.get("then").unwrap_or(&Value::Null);
                    let else_p = obj.get("else").unwrap_or(&Value::Null);
                    let b = eval_bool(root_snapshot, self_path, pred, scope)?;
                    let branch = if b { then_p } else { else_p };
                    eval_plan(root_snapshot, self_path, branch, scope)
                }

                // Booléens/comparateurs → via eval_bool
                "and" | "or" | "not" | "lt" | "le" | "gt" | "ge" | "eq" | "ne" => {
                    let b = eval_bool(root_snapshot, self_path, plan, scope)?;
                    Ok(json!(b))
                }

                _ => Err(anyhow!("unsupported op: {}", op)),
            };
        }
    }
    Ok(plan.clone())
}

/* =========================================================================================
 *  Overlay de snapshot (fonctionnelle, sans emprunts mutables imbriqués)
 * =======================================================================================*/

/// Retourne une copie de `base_root` où le sous-arbre à `path` est remplacé par `subtree`.
fn with_subtree(base_root: &Value, path: &[String], subtree: &Value) -> Value {
    fn rec(node: &Value, path: &[String], subtree: &Value) -> Value {
        if path.is_empty() {
            return subtree.clone();
        }
        let key = &path[0];
        let rest = &path[1..];

        let mut obj: Map<String, Value> = match node {
            Value::Object(m) => m.clone(),
            _ => Map::new(),
        };

        let child = obj.get(key).cloned().unwrap_or(Value::Null);
        let new_child = rec(&child, rest, subtree);
        obj.insert(key.clone(), new_child);
        Value::Object(obj)
    }
    rec(base_root, path, subtree)
}

/* =========================================================================================
 *  x_compute
 * =======================================================================================*/

fn compute_value_for_node(
    root_snapshot: &Value,
    current_value: &Value,
    schema: &Value,
    path: &[String],
) -> Result<Option<Value>> {
    let Some(xc_v) = schema.get("x_compute") else {
        return Ok(None);
    };
    let Some(xc) = xc_v.as_object() else {
        return Ok(None);
    };

    let engine = xc
        .get("engine")
        .and_then(|v| v.as_str())
        .unwrap_or("plan/v1");
    if engine != "plan/v1" {
        return Ok(None);
    }

    let scope = parse_scope(xc.get("scope").and_then(|v| v.as_str()));
    let update = parse_update(xc.get("update").and_then(|v| v.as_str()));
    // forme courte: si "plan" absent → prendre tout l'objet x_compute
    let plan: &Value = xc.get("plan").unwrap_or(xc_v);

    // Politique d’écriture
    let mut need_write = match update {
        Update::Always => true,
        Update::IfMissing => current_value.is_null(),
        Update::IfNull => current_value.is_null(),
    };

    // placeholders (IfMissing)
    if !need_write && matches!(update, Update::IfMissing) {
        if let Some(op) = plan.get("op").and_then(|v| v.as_str()) {
            if let Some(s) = current_value.as_str() {
                if (op == "uuid_v4" && s == "00000000-0000-0000-0000-000000000000")
                    || (op == "now_rfc3339" && s == "1970-01-01T00:00:00Z")
                {
                    need_write = true;
                }
            }
        }
    }

    if !need_write {
        return Ok(None);
    }

    // Base path (parent si scope=self pour viser les frères)
    let parent_path: &[String] = if matches!(scope, Scope::Self_) && !path.is_empty() {
        &path[..path.len() - 1]
    } else {
        path
    };

    // On évalue contre le snapshot overlayé (déjà préparé plus bas dans apply_xc_rec)
    let computed = eval_plan(root_snapshot, parent_path, plan, scope)?;
    Ok(Some(computed))
}

/// Public API : applique x_compute jusqu'à convergence (4 passes max).
pub fn apply_x_compute(instance: &mut Value, schema: &Value) -> Result<()> {
    for _ in 0..MAX_PASSES.with(|c| c.get()) {
        let snapshot = instance.clone();
        let mut changed = false;
        apply_xc_rec(&snapshot, instance, schema, &mut Vec::new(), &mut changed)?;
        if !changed {
            break;
        }
    }
    Ok(())
}

fn apply_xc_rec(
    snapshot_root: &Value,
    node: &mut Value,
    schema: &Value,
    path: &mut Vec<String>,
    changed: &mut bool,
) -> Result<()> {
    // Combinateurs
    if let Some(list) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub in list {
            apply_xc_rec(snapshot_root, node, sub, path, changed)?;
        }
    }
    if let Some(list) = schema
        .get("oneOf")
        .and_then(|v| v.as_array())
        .or_else(|| schema.get("anyOf").and_then(|v| v.as_array()))
    {
        if let Some(first) = list.first() {
            apply_xc_rec(snapshot_root, node, first, path, changed)?;
        }
    }

    // x_compute sur le nœud courant
    if let Some(new_val) = compute_value_for_node(snapshot_root, node, schema, path)? {
        *node = new_val;
        *changed = true;
    }

    // Descente
    match schema.get("type").and_then(|v| v.as_str()) {
        Some("object") => {
            if !node.is_object() {
                *node = Value::Object(Map::new());
            }
            let map_ref = node.as_object_mut().expect("object");

            // required
            let required: Vec<String> = schema
                .get("required")
                .and_then(|v| v.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();

            // helper défauts
            let pick_default_like = |sub: &Value| -> Option<Value> {
                if let Some(c) = sub.get("const") {
                    return Some(c.clone());
                }
                if let Some(e) = sub.get("enum").and_then(|v| v.as_array()) {
                    if let Some(first) = e.first() {
                        return Some(first.clone());
                    }
                }
                if let Some(d) = sub.get("default") {
                    return Some(d.clone());
                }
                None
            };

            // 1) Insérer const/enum[0]/default pour les clés manquantes
            let mut to_insert: Vec<(String, Value)> = Vec::new();
            if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
                for (k, sub_schema) in props {
                    if !map_ref.contains_key(k) {
                        if let Some(defv) = pick_default_like(sub_schema) {
                            to_insert.push((k.clone(), defv));
                        }
                    }
                }
            }
            if !to_insert.is_empty() {
                for (k, v) in to_insert {
                    map_ref.insert(k, v);
                }
                *changed = true;
            }

            // 2) Overlay du snapshot au niveau courant
            //    → on clone l’objet pour éviter les emprunts simultanés (mut + immu)
            let current_object_value = Value::Object(map_ref.clone());
            let overlay_snapshot = with_subtree(snapshot_root, path, &current_object_value);

            // 3) Recurse sur chaque propriété (présentes OU nécessaires)
            if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
                // Collecter les clés à traiter (copie pour éviter d’itérer pendant mut)
                let mut keys: Vec<String> = map_ref.keys().cloned().collect();
                for (k, sub_schema) in props {
                    if !keys.contains(k)
                        && (required.iter().any(|r| r == k)
                            || schema_has_x_compute(sub_schema)
                            || schema_has_default_like(sub_schema))
                    {
                        keys.push(k.clone());
                    }
                }

                // Pour éviter le prêt immuable d’éléments pendant que map_ref est mut,
                // on procède clé par clé en retirant puis réinsérant.
                for k in keys {
                    if props.get(&k).is_none() {
                        continue;
                    }
                    let sub_schema = props.get(&k).unwrap();

                    // retirer (si présent) pour libérer le prêt mut de map_ref[k]
                    let mut child = map_ref.remove(&k).unwrap_or(Value::Null);

                    path.push(k.clone());
                    apply_xc_rec(&overlay_snapshot, &mut child, sub_schema, path, changed)?;
                    path.pop();

                    if !child.is_null() {
                        map_ref.insert(k, child);
                    }
                }
            }
        }
        Some("array") => {
            // Si le noeud n'est pas un tableau, on ne tente pas d'y descendre.
            if !node.is_array() {
                return Ok(());
            }
            let items_schema = match schema.get("items") {
                Some(s) => s,
                None => return Ok(()),
            };
            let arr = node.as_array_mut().expect("array");

            // Itérer par index, en évitant les prêts multiples :
            // - cloner l’élément pour composer le snapshot overlay
            // - puis repasser &mut sur l’élément
            for i in 0..arr.len() {
                let elem_clone = arr[i].clone();
                let mut elem_path = path.clone();
                elem_path.push(i.to_string());

                let overlay_snapshot = with_subtree(snapshot_root, &elem_path, &elem_clone);

                path.push(i.to_string());
                apply_xc_rec(&overlay_snapshot, &mut arr[i], items_schema, path, changed)?;
                path.pop();
            }
        }
        _ => {}
    }

    Ok(())
}

/// Variante avec options (max_passes, strict_ptr).
pub fn apply_x_compute_with_opts(
    instance: &mut Value,
    schema: &Value,
    opts: ComputeOptions,
) -> Result<()> {
    // sauvegarde et set
    let prev_max = MAX_PASSES.with(|c| {
        let p = c.get();
        c.set(opts.max_passes);
        p
    });
    let prev_strict = STRICT_PTR.with(|c| {
        let p = c.get();
        c.set(opts.strict_ptr);
        p
    });
    let res = apply_x_compute(instance, schema);
    // restauration
    MAX_PASSES.with(|c| c.set(prev_max));
    STRICT_PTR.with(|c| c.set(prev_strict));
    res
}
