// FICHIER : src-tauri/src/json_db/schema/compute.rs

use anyhow::{anyhow, Result};
use chrono::Utc;
use serde_json::{json, Value};
use std::cell::Cell;
use std::path::{Component, Path, PathBuf};
use uuid::Uuid;

use super::registry::SchemaRegistry;

thread_local! {
    static MAX_PASSES: Cell<usize> = const { Cell::new(4) };
}

#[derive(Clone, Copy, Debug)]
pub struct ComputeOptions {
    pub max_passes: usize,
    pub strict_ptr: bool,
}

#[derive(Copy, Clone, PartialEq)]
enum UpdateMode {
    Always,
    IfMissing,
    IfNull,
}

#[derive(Copy, Clone, PartialEq, Debug)]
enum Scope {
    Root,
    SelfRelative,
}

pub fn apply_x_compute(
    instance: &mut Value,
    schema: &Value,
    registry: &SchemaRegistry,
    root_uri: &str,
) -> Result<()> {
    let opts = ComputeOptions {
        max_passes: 4,
        strict_ptr: false,
    };
    apply_x_compute_with_opts(instance, schema, registry, root_uri, opts)
}

pub fn apply_x_compute_with_opts(
    instance: &mut Value,
    schema: &Value,
    registry: &SchemaRegistry,
    root_uri: &str,
    opts: ComputeOptions,
) -> Result<()> {
    let prev_max = MAX_PASSES.with(|c| {
        let p = c.get();
        c.set(opts.max_passes);
        p
    });

    let max = MAX_PASSES.with(|c| c.get());
    for _ in 0..max {
        let snapshot = instance.clone();
        let mut changed = false;
        let mut path = Vec::new();

        apply_xc_rec(
            &snapshot,
            instance,
            schema,
            registry,
            root_uri,
            &mut path,
            &mut changed,
        )?;

        if !changed {
            break;
        }
    }

    MAX_PASSES.with(|c| c.set(prev_max));
    Ok(())
}

fn apply_xc_rec(
    snapshot_root: &Value,
    node: &mut Value,
    schema: &Value,
    registry: &SchemaRegistry,
    current_uri: &str,
    path: &mut Vec<String>,
    changed: &mut bool,
) -> Result<()> {
    // Priorité aux Defaults : On applique d'abord les valeurs par défaut
    if node.is_null() {
        if let Some(default_val) = schema.get("default") {
            *node = default_val.clone();
            *changed = true;
        }
    }

    // 1. GESTION DES RÉFÉRENCES (AVEC DEBUGGING)
    if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        let (file_uri, fragment) = if ref_str.starts_with('#') {
            (current_uri.to_string(), Some(ref_str.to_string()))
        } else {
            let resolved = resolve_path_uri(current_uri, ref_str);
            let (f, frag) = split_uri_fragment(&resolved);
            (f.to_string(), frag.map(|s| s.to_string()))
        };

        if let Some(target_schema_root) = registry.get_by_uri(&file_uri) {
            let target_schema = if let Some(frag) = fragment {
                let pointer = frag.replace('#', "");
                target_schema_root
                    .pointer(&pointer)
                    .unwrap_or(target_schema_root)
            } else {
                target_schema_root
            };

            return apply_xc_rec(
                snapshot_root,
                node,
                target_schema,
                registry,
                &file_uri,
                path,
                changed,
            );
        } else {
            // --- BLOC DE DIAGNOSTIC AJOUTÉ ---
            #[cfg(test)] // Ce bloc ne s'active que lors des tests (cargo test)
            {
                // On affiche ce message seulement si on cherche un schéma "base.schema.json"
                // pour éviter de spammer les logs pour d'autres refs.
                if ref_str.contains("base.schema.json") {
                    println!("⚠️ [x_compute] Échec résolution $ref critique !");
                    println!("   Source URI   : {}", current_uri);
                    println!("   Ref brute    : {}", ref_str);
                    println!("   URI Calculée : {}", file_uri);

                    // On liste les clés proches dans le registre pour comprendre l'écart
                    // (Nécessite la méthode list_uris ajoutée précédemment dans Registry)
                    let available_keys = registry.list_uris();
                    println!("   Registre ({} clés) :", available_keys.len());

                    let mut found_candidate = false;
                    for k in available_keys {
                        if k.contains("base.schema.json") {
                            println!("   - Candidat trouvé : {}", k);
                            found_candidate = true;
                        }
                    }
                    if !found_candidate {
                        println!(
                            "   - AUCUN candidat trouvé pour base.schema.json (Fichier manquant ?)"
                        );
                    }
                }
            }
            return Ok(());
        }
    }

    // 2. INITIALISATION STRUCTURELLE
    if node.is_null() {
        if let Some(t) = schema.get("type").and_then(|t| t.as_str()) {
            if t == "object" {
                *node = Value::Object(serde_json::Map::new());
                *changed = true;
            } else if t == "array" {
                *node = Value::Array(Vec::new());
                *changed = true;
            }
        }
    }

    // 3. RÈGLE X_COMPUTE
    if let Some(xc) = schema.get("x_compute") {
        apply_rule_def(snapshot_root, node, xc, path, changed)?;
    }

    // 4. PROPRIÉTÉS (Objets)
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        if let Some(obj) = node.as_object_mut() {
            for (key, sub_schema) in props {
                let mut created_temp = false;

                if !obj.contains_key(key) && needs_computation(sub_schema) {
                    obj.insert(key.clone(), Value::Null);
                    created_temp = true;
                }

                if let Some(sub_node) = obj.get_mut(key) {
                    path.push(key.clone());
                    apply_xc_rec(
                        snapshot_root,
                        sub_node,
                        sub_schema,
                        registry,
                        current_uri,
                        path,
                        changed,
                    )?;
                    path.pop();

                    if created_temp && sub_node.is_null() {
                        obj.remove(key);
                    } else if created_temp {
                        *changed = true;
                    }
                }
            }
        }
    }
    // 5. ITEMS (Tableaux)
    else if let Some(items_schema) = schema.get("items") {
        if let Some(arr) = node.as_array_mut() {
            for (i, sub_node) in arr.iter_mut().enumerate() {
                path.push(i.to_string());
                apply_xc_rec(
                    snapshot_root,
                    sub_node,
                    items_schema,
                    registry,
                    current_uri,
                    path,
                    changed,
                )?;
                path.pop();
            }
        }
    }

    // 6. ALL OF
    if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub in all_of {
            apply_xc_rec(
                snapshot_root,
                node,
                sub,
                registry,
                current_uri,
                path,
                changed,
            )?;
        }
    }
    Ok(())
}

fn apply_rule_def(
    snapshot_root: &Value,
    node: &mut Value,
    rule: &Value,
    current_path: &[String],
    changed: &mut bool,
) -> Result<()> {
    let update_str = rule.get("update").and_then(|s| s.as_str());
    let mode = match update_str {
        Some("always") => UpdateMode::Always,
        Some("if_null") => UpdateMode::IfNull,
        _ => UpdateMode::IfMissing,
    };

    let should_update = match mode {
        UpdateMode::Always => true,
        UpdateMode::IfMissing => node.is_null(),
        UpdateMode::IfNull => node.is_null(),
    };

    if !should_update {
        return Ok(());
    }

    let scope_str = rule.get("scope").and_then(|s| s.as_str()).unwrap_or("root");
    let scope = if scope_str == "self" {
        Scope::SelfRelative
    } else {
        Scope::Root
    };

    let self_ptr = if !current_path.is_empty() {
        let parent_path = &current_path[..current_path.len() - 1];
        if parent_path.is_empty() {
            "".to_string()
        } else {
            "/".to_string() + &parent_path.join("/")
        }
    } else {
        "".to_string()
    };

    if let Some(plan) = rule.get("plan") {
        match evaluate_plan(plan, snapshot_root, current_path, scope, &self_ptr) {
            Ok(result) => {
                if *node != result {
                    *node = result;
                    *changed = true;
                }
            }
            Err(_e) => {}
        }
    }
    Ok(())
}

fn evaluate_plan(
    plan: &Value,
    root: &Value,
    current_path: &[String],
    scope: Scope,
    self_ptr: &str,
) -> Result<Value> {
    let obj = match plan.as_object() {
        Some(o) => o,
        None => return Ok(plan.clone()),
    };

    if let Some(ptr) = obj.get("ptr").and_then(|v| v.as_str()) {
        return resolve_pointer(ptr, root, current_path, scope, self_ptr);
    }

    if let Some(op) = obj.get("op").and_then(|v| v.as_str()) {
        let args = obj.get("args").and_then(|v| v.as_array());
        let mut evaluated_args = Vec::new();

        if let Some(args_arr) = args {
            if op != "sum" {
                for arg in args_arr {
                    evaluated_args.push(evaluate_plan(arg, root, current_path, scope, self_ptr)?);
                }
            }
        }

        return match op {
            "uuid_v4" => Ok(Value::String(Uuid::new_v4().to_string())),
            "now_utc" | "now_rfc3339" => Ok(Value::String(Utc::now().to_rfc3339())),
            "const" => Ok(obj.get("value").unwrap_or(&Value::Null).clone()),
            "add" => op_math_reduce(&evaluated_args, |a, b| a + b),
            "sub" => op_math_reduce(&evaluated_args, |a, b| a - b),
            "mul" => op_math_reduce(&evaluated_args, |a, b| a * b),
            "div" => op_math_reduce(&evaluated_args, |a, b| if b != 0.0 { a / b } else { 0.0 }),
            "round" => op_round(&evaluated_args, obj),
            "sum" => op_sum(obj, root, current_path, scope, self_ptr),
            "eq" => Ok(Value::Bool(evaluated_args.first() == evaluated_args.get(1))),
            "ne" => Ok(Value::Bool(evaluated_args.first() != evaluated_args.get(1))),
            "gt" => op_compare(&evaluated_args, |a, b| a > b),
            "gte" => op_compare(&evaluated_args, |a, b| a >= b),
            "lt" => op_compare(&evaluated_args, |a, b| a < b),
            "lte" | "le" => op_compare(&evaluated_args, |a, b| a <= b),
            "not" => {
                let val = evaluated_args
                    .first()
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                Ok(Value::Bool(!val))
            }
            "and" => {
                let res = evaluated_args.iter().all(|v| v.as_bool().unwrap_or(false));
                Ok(Value::Bool(res))
            }
            "or" => {
                let res = evaluated_args.iter().any(|v| v.as_bool().unwrap_or(false));
                Ok(Value::Bool(res))
            }
            _ => Err(anyhow!("Opération inconnue: {}", op)),
        };
    }

    Ok(Value::Object(obj.clone()))
}

fn op_math_reduce<F>(args: &[Value], op: F) -> Result<Value>
where
    F: Fn(f64, f64) -> f64,
{
    if args.is_empty() {
        return Ok(json!(0.0));
    }
    let mut acc = args[0].as_f64().unwrap_or(0.0);
    for val in &args[1..] {
        acc = op(acc, val.as_f64().unwrap_or(0.0));
    }
    Ok(json!(acc))
}

fn op_round(args: &[Value], config: &serde_json::Map<String, Value>) -> Result<Value> {
    let val = args.first().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let scale = config.get("scale").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let multiplier = 10f64.powi(scale);
    let rounded = (val * multiplier).round() / multiplier;
    Ok(json!(rounded))
}

fn op_compare<F>(args: &[Value], op: F) -> Result<Value>
where
    F: Fn(f64, f64) -> bool,
{
    if args.len() < 2 {
        return Ok(Value::Bool(false));
    }
    let a = args[0].as_f64().unwrap_or(0.0);
    let b = args[1].as_f64().unwrap_or(0.0);
    Ok(Value::Bool(op(a, b)))
}

fn op_sum(
    config: &serde_json::Map<String, Value>,
    root: &Value,
    current_path: &[String],
    scope: Scope,
    self_ptr: &str,
) -> Result<Value> {
    let from_ptr = config.get("from").and_then(|v| v.as_str()).unwrap_or("");
    let field_path = config.get("path").and_then(|v| v.as_str()).unwrap_or("");
    let source_array = resolve_pointer(from_ptr, root, current_path, scope, self_ptr)?;
    let mut sum = 0.0;
    if let Some(arr) = source_array.as_array() {
        for item in arr {
            let val = if field_path.is_empty() {
                item
            } else {
                item.get(field_path).unwrap_or(&Value::Null)
            };
            sum += val.as_f64().unwrap_or(0.0);
        }
    }
    Ok(json!(sum))
}

fn resolve_pointer(
    ptr: &str,
    root: &Value,
    current_path: &[String],
    scope: Scope,
    self_ptr: &str,
) -> Result<Value> {
    if ptr.starts_with("#/") {
        let raw_ptr = &ptr[1..];
        let final_ptr = if scope == Scope::SelfRelative {
            if self_ptr.is_empty() {
                raw_ptr.to_string()
            } else {
                format!("{}{}", self_ptr, raw_ptr)
            }
        } else {
            raw_ptr.to_string()
        };
        return root
            .pointer(&final_ptr)
            .cloned()
            .ok_or_else(|| anyhow!("Pointeur introuvable: {}", final_ptr));
    }
    if ptr.starts_with("#/../") {
        let parts: Vec<&str> = ptr.split('/').collect();
        let mut effective_path = current_path.to_vec();
        let mut i = 1;
        while i < parts.len() && parts[i] == ".." {
            effective_path.pop();
            i += 1;
        }
        while i < parts.len() {
            effective_path.push(parts[i].to_string());
            i += 1;
        }
        let abs_ptr = if effective_path.is_empty() {
            "".to_string()
        } else {
            "/".to_string() + &effective_path.join("/")
        };
        return root
            .pointer(&abs_ptr)
            .cloned()
            .ok_or_else(|| anyhow!("Pointeur relatif introuvable: {}", abs_ptr));
    }
    Ok(Value::String(ptr.to_string()))
}

fn needs_computation(schema: &Value) -> bool {
    schema.get("x_compute").is_some()
        || schema.get("default").is_some()
        || schema.get("$ref").is_some()
        || schema.get("properties").is_some()
        || schema.get("allOf").is_some()
}

fn split_uri_fragment(uri: &str) -> (&str, Option<&str>) {
    if let Some(idx) = uri.find('#') {
        (&uri[0..idx], Some(&uri[idx..]))
    } else {
        (uri, None)
    }
}

fn resolve_path_uri(base: &str, target_path: &str) -> String {
    if target_path.starts_with("db://") {
        return target_path.to_string();
    }
    if target_path.is_empty() {
        return base.to_string();
    }
    let (prefix, base_path_str) = if let Some(stripped) = base.strip_prefix("db://") {
        ("db://", stripped)
    } else {
        ("", base)
    };
    let base_path = Path::new(base_path_str);
    let parent = base_path.parent().unwrap_or(Path::new(""));
    let joined = parent.join(target_path);
    let normalized = normalize_path(&joined);
    format!(
        "{}{}",
        prefix,
        normalized.to_string_lossy().replace("\\", "/")
    )
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop();
            }
            Component::Normal(c) => components.push(c),
            Component::RootDir | Component::Prefix(_) => {}
        }
    }
    let mut result = PathBuf::new();
    for c in components {
        result.push(c);
    }
    result
}
