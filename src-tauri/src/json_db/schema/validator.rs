use anyhow::{anyhow, Result};
use serde_json::{Map, Value};

use super::compute::{apply_x_compute_with_opts, ComputeOptions};
use super::registry::SchemaRegistry;

#[derive(Debug, Clone)]
pub struct SchemaValidator {
    root_uri: String,
    schema: Value,
    reg: SchemaRegistry,
}

impl SchemaValidator {
    /// Compile en résolvant le schéma racine via le registre (sans fetch externe).
    pub fn compile_with_registry(root_uri: &str, reg: &SchemaRegistry) -> Result<Self> {
        let schema = reg
            .get_by_uri(root_uri)
            .cloned()
            .ok_or_else(|| anyhow!("Schema not found in registry: {}", root_uri))?;
        Ok(Self {
            root_uri: root_uri.to_string(),
            schema,
            reg: reg.clone(),
        })
    }

    /// Construit un schéma “expansé” au 1er niveau de `properties` :
    /// si une propriété est un pur {$ref}, on remplace par la cible déréférencée.
    fn expand_properties_refs(&self) -> Result<Value> {
        let mut expanded = self.schema.clone();
        let props = self
            .schema
            .get("properties")
            .and_then(|v| v.as_object())
            .ok_or_else(|| anyhow!("schema has no properties"))?;

        let mut new_props = Map::new();
        for (name, prop_schema) in props {
            if let Some(r) = prop_schema.get("$ref").and_then(|v| v.as_str()) {
                let (_u, node) = self.reg.resolve_ref(&self.root_uri, r)?;
                new_props.insert(name.clone(), node);
            } else {
                new_props.insert(name.clone(), prop_schema.clone());
            }
        }
        expanded
            .as_object_mut()
            .ok_or_else(|| anyhow!("schema root is not an object"))?
            .insert("properties".into(), Value::Object(new_props));
        Ok(expanded)
    }

    /// Applique x_compute (sur schéma avec refs de propriétés expansées), puis valide.
    pub fn compute_then_validate(&self, instance: &mut Value) -> Result<()> {
        let expanded_for_compute = self.expand_properties_refs()?;
        prefill_dollar_schema(instance, &expanded_for_compute, &self.root_uri);
        apply_x_compute_with_opts(
            instance,
            &expanded_for_compute,
            ComputeOptions {
                max_passes: 4,
                strict_ptr: false,
            },
        )?;
        self.validate(instance)
    }

    /// Validation minimale (types / required / properties / additionalProperties / enum / minLength / items / minItems).
    pub fn validate(&self, instance: &Value) -> Result<()> {
        validate_against(&self.schema, instance).map_err(|e| anyhow!(e))
    }

    pub fn compiled_schema(&self) -> &Value {
        &self.schema
    }
}

/* ----------------------------- Helpers $ref (optionnels) ------------------------------ */

fn split_fragment(uri: &str) -> (&str, Option<&str>) {
    if let Some(idx) = uri.find('#') {
        (&uri[..idx], Some(&uri[idx..]))
    } else {
        (uri, None)
    }
}

/// Résolution récursive des $ref (JSON Pointer supporté).
#[allow(dead_code)]
fn resolve_refs(node: &Value, base_uri: &str, reg: &SchemaRegistry) -> Result<Value> {
    match node {
        Value::Object(map) => {
            if let Some(Value::String(r)) = map.get("$ref") {
                let (ref_path, frag) = if let Some(idx) = r.find('#') {
                    (&r[..idx], Some(&r[idx + 1..]))
                } else {
                    (r.as_str(), None)
                };

                let target_uri = if ref_path.is_empty() {
                    let (base_no_frag, _f) = split_fragment(base_uri);
                    base_no_frag.to_string()
                } else {
                    reg.join(base_uri, ref_path)?
                };

                let doc = reg
                    .get_by_uri(&target_uri)
                    .ok_or_else(|| anyhow!("$ref not found in registry: {}", target_uri))?
                    .clone();

                let mut target = doc;
                if let Some(pointer) = frag {
                    let ptr = if pointer.starts_with('/') {
                        pointer.to_string()
                    } else {
                        format!("/{}", pointer)
                    };
                    target = target.pointer(&ptr).cloned().ok_or_else(|| {
                        anyhow!("Fragment introuvable: {} in {}", ptr, target_uri)
                    })?;
                }

                let mut resolved = resolve_refs(&target, &target_uri, reg)?;
                for (k, v) in map.iter() {
                    if k != "$ref" {
                        merge(&mut resolved, &resolve_refs(v, base_uri, reg)?);
                    }
                }
                Ok(resolved)
            } else {
                let mut out = Map::with_capacity(map.len());
                for (k, v) in map {
                    out.insert(k.clone(), resolve_refs(v, base_uri, reg)?);
                }
                Ok(Value::Object(out))
            }
        }
        Value::Array(arr) => {
            let mut out = Vec::with_capacity(arr.len());
            for v in arr {
                out.push(resolve_refs(v, base_uri, reg)?);
            }
            Ok(Value::Array(out))
        }
        _ => Ok(node.clone()),
    }
}

/// Fusion profonde d’objets JSON (b écrase a).
fn merge(a: &mut Value, b: &Value) {
    match (a, b) {
        (Value::Object(ao), Value::Object(bo)) => {
            for (k, v) in bo {
                match ao.get_mut(k) {
                    Some(av) => merge(av, v),
                    None => {
                        ao.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (a @ _, b) => {
            *a = b.clone();
        }
    }
}

/// Si la propriété `$schema` est déclarée dans le schéma et absente de l'instance,
/// on la remplit depuis const/enum/default ; à défaut, on met l'URI du schéma racine.
fn prefill_dollar_schema(instance: &mut Value, expanded_schema: &Value, root_uri: &str) {
    // On n'agit que sur des objets
    let obj = match instance.as_object_mut() {
        Some(o) => o,
        None => return,
    };
    // Si déjà présent, ne rien faire
    if obj.contains_key("$schema") {
        return;
    }
    // La propriété $schema existe-t-elle dans le schéma ?
    let props = match expanded_schema
        .get("properties")
        .and_then(|v| v.as_object())
    {
        Some(p) => p,
        None => return,
    };
    let Some(pdef) = props.get("$schema") else {
        return; // le schéma ne déclare pas $schema → on ne force rien
    };

    // Priorités: const -> enum[0] -> default -> fallback root_uri
    let candidate = pdef
        .get("const")
        .cloned()
        .or_else(|| {
            pdef.get("enum")
                .and_then(|e| e.as_array())
                .and_then(|a| a.first().cloned())
        })
        .or_else(|| pdef.get("default").cloned())
        .unwrap_or_else(|| Value::String(root_uri.to_string()));

    obj.insert("$schema".to_string(), candidate);
}

/* -------------------- Validateur minimal (sans lib JSON externe) ---------------------- */

fn validate_against(schema: &Value, value: &Value) -> Result<(), String> {
    if let Value::Object(map) = schema {
        // type
        if let Some(Value::String(t)) = map.get("type") {
            match t.as_str() {
                "object" => {
                    let obj = value.as_object().ok_or_else(|| {
                        format!("Type mismatch: expected object, got {}", kind_of(value))
                    })?;
                    // required
                    if let Some(Value::Array(reqs)) = map.get("required") {
                        for r in reqs {
                            if let Some(name) = r.as_str() {
                                if !obj.contains_key(name) {
                                    return Err(format!("Missing required property: {}", name));
                                }
                            }
                        }
                    }
                    // properties
                    let mut known = std::collections::HashSet::<&str>::new();
                    if let Some(Value::Object(props)) = map.get("properties") {
                        for (k, sub) in props {
                            if let Some(vsub) = obj.get(k) {
                                validate_against(sub, vsub)?;
                            }
                            known.insert(k.as_str());
                        }
                    }
                    // additionalProperties
                    if let Some(ap) = map.get("additionalProperties") {
                        match ap {
                            Value::Bool(false) => {
                                for k in obj.keys() {
                                    if !known.contains(k.as_str()) {
                                        return Err(format!(
                                            "Additional property not allowed: {}",
                                            k
                                        ));
                                    }
                                }
                            }
                            Value::Object(_) => {
                                for (k, v) in obj {
                                    if !known.contains(k.as_str()) {
                                        validate_against(ap, v)?;
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                "array" => {
                    let arr = value.as_array().ok_or_else(|| {
                        format!("Type mismatch: expected array, got {}", kind_of(value))
                    })?;
                    if let Some(Value::Number(n)) = map.get("minItems") {
                        if let Some(min) = n.as_u64() {
                            if arr.len() < min as usize {
                                return Err(format!("Array has fewer than minItems ({})", min));
                            }
                        }
                    }
                    if let Some(items) = map.get("items") {
                        for v in arr {
                            validate_against(items, v)?;
                        }
                    }
                }
                "string" => {
                    if !value.is_string() {
                        return Err(format!(
                            "Type mismatch: expected string, got {}",
                            kind_of(value)
                        ));
                    }
                    if let Some(Value::Number(n)) = map.get("minLength") {
                        if let Some(min) = n.as_u64() {
                            if value.as_str().unwrap().chars().count() < min as usize {
                                return Err(format!("String shorter than minLength ({})", min));
                            }
                        }
                    }
                    if let Some(Value::Array(en)) = map.get("enum") {
                        let s = value.as_str().unwrap();
                        let ok = en.iter().any(|e| e.as_str() == Some(s));
                        if !ok {
                            return Err(format!("Value '{}' not in enum", s));
                        }
                    }
                }
                "number" => {
                    if !value.is_number() {
                        return Err(format!(
                            "Type mismatch: expected number, got {}",
                            kind_of(value)
                        ));
                    }
                }
                "integer" => {
                    if !(value.is_i64() || value.is_u64()) {
                        return Err(format!(
                            "Type mismatch: expected integer, got {}",
                            kind_of(value)
                        ));
                    }
                }
                "boolean" => {
                    if !value.is_boolean() {
                        return Err(format!(
                            "Type mismatch: expected boolean, got {}",
                            kind_of(value)
                        ));
                    }
                }
                "null" => {
                    if !value.is_null() {
                        return Err(format!(
                            "Type mismatch: expected null, got {}",
                            kind_of(value)
                        ));
                    }
                }
                _ => { /* types custom ignorés */ }
            }
        }

        // enum au niveau courant (tous types)
        if let Some(Value::Array(en)) = map.get("enum") {
            let ok = en.iter().any(|e| e == value);
            if !ok {
                return Err("Value not in enum set".to_string());
            }
        }
    }
    Ok(())
}

fn kind_of(v: &Value) -> &'static str {
    if v.is_object() {
        "object"
    } else if v.is_array() {
        "array"
    } else if v.is_string() {
        "string"
    } else if v.is_boolean() {
        "boolean"
    } else if v.is_null() {
        "null"
    } else if v.is_i64() || v.is_u64() {
        "integer"
    } else if v.is_number() {
        "number"
    } else {
        "unknown"
    }
}
