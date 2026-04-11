// FICHIER : src-tauri/src/json_db/schema/validator.rs
use crate::utils::prelude::*;

use super::registry::SchemaRegistry;

#[derive(Debug, Clone)]
pub struct SchemaValidator {
    root_uri: String,
    schema: JsonValue,
    reg: SchemaRegistry,
}

impl SchemaValidator {
    pub fn compile_with_registry(root_uri: &str, reg: &SchemaRegistry) -> RaiseResult<Self> {
        let Some(schema) = reg.get_by_uri(root_uri).cloned() else {
            raise_error!(
                "ERR_SCHEMA_NOT_IN_REGISTRY",
                error = format!("Le schéma sémantique est introuvable : {}", root_uri),
                context = json_value!({
                    "root_uri": root_uri,
                    "action": "resolve_schema_from_registry",
                    "hint": "Vérifiez que l'ontologie a été correctement chargée."
                })
            );
        };

        Ok(Self {
            root_uri: root_uri.to_string(),
            schema,
            reg: reg.clone(),
        })
    }

    pub fn compute_then_validate(&self, instance: &mut JsonValue) -> RaiseResult<()> {
        apply_defaults(instance, &self.schema, &self.reg, &self.root_uri)?;
        self.validate(instance)
    }

    pub fn validate(&self, instance: &JsonValue) -> RaiseResult<()> {
        validate_node(instance, &self.schema, &self.reg, &self.root_uri)
    }
}

fn resolve_schema_node<'a>(
    schema: &'a JsonValue,
    reg: &'a SchemaRegistry,
    current_uri: &str,
) -> &'a JsonValue {
    if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        let (file_uri, fragment) = if ref_str.starts_with('#') {
            (current_uri.to_string(), Some(ref_str.to_string()))
        } else {
            let resolved = resolve_path_uri(current_uri, ref_str);
            let (f, frag) = split_uri_fragment(&resolved);
            (f.to_string(), frag.map(|s| s.to_string()))
        };

        if let Some(target_root) = reg.get_by_uri(&file_uri) {
            if let Some(frag) = fragment {
                return target_root
                    .pointer(&frag.replace('#', ""))
                    .unwrap_or(schema);
            }
            return target_root;
        }
    }
    schema
}

fn apply_defaults(
    instance: &mut JsonValue,
    schema: &JsonValue,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        let (file_uri, fragment) = if ref_str.starts_with('#') {
            (current_uri.to_string(), Some(ref_str.to_string()))
        } else {
            let resolved = resolve_path_uri(current_uri, ref_str);
            let (f, frag) = split_uri_fragment(&resolved);
            (f.to_string(), frag.map(|s| s.to_string()))
        };

        if let Some(target_root) = reg.get_by_uri(&file_uri) {
            let target_schema = if let Some(frag) = fragment {
                target_root
                    .pointer(&frag.replace('#', ""))
                    .unwrap_or(target_root)
            } else {
                target_root
            };
            return apply_defaults(instance, target_schema, reg, &file_uri);
        }
    }

    if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub_schema in all_of {
            apply_defaults(instance, sub_schema, reg, current_uri)?;
        }
    }

    if let Some(obj) = instance.as_object_mut() {
        if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
            for (key, sub_schema) in props {
                let resolved_schema = resolve_schema_node(sub_schema, reg, current_uri);
                let is_missing = obj.get(key).is_none_or(|v| v.is_null());

                if is_missing {
                    let default_val = sub_schema
                        .get("default")
                        .or_else(|| resolved_schema.get("default"));

                    if let Some(val) = default_val {
                        obj.insert(key.clone(), val.clone());
                    }
                }

                let compute_node = sub_schema
                    .get("x_compute")
                    .or_else(|| resolved_schema.get("x_compute"))
                    .and_then(|v| v.as_object());

                if let Some(compute) = compute_node {
                    let update_strategy = compute
                        .get("update")
                        .and_then(|v| v.as_str())
                        .unwrap_or("if_missing");

                    let is_currently_missing = obj.get(key).is_none_or(|v| v.is_null());

                    let should_compute = match update_strategy {
                        "always" => true,
                        "if_missing" => is_currently_missing,
                        _ => false,
                    };

                    if should_compute {
                        if let Some(plan) = compute.get("plan").and_then(|v| v.as_object()) {
                            if let Some(op) = plan.get("op").and_then(|v| v.as_str()) {
                                let computed_val = execute_compute_plan(op, plan);
                                if !computed_val.is_null() {
                                    obj.insert(key.clone(), computed_val);
                                }
                            }
                        }
                    }
                }

                if let Some(val) = obj.get_mut(key) {
                    apply_defaults(val, sub_schema, reg, current_uri)?;
                }
            }
        }
    }

    if let Some(arr) = instance.as_array_mut() {
        if let Some(items_schema) = schema.get("items") {
            for item in arr {
                apply_defaults(item, items_schema, reg, current_uri)?;
            }
        }
    }

    Ok(())
}

fn validate_node(
    instance: &JsonValue,
    schema: &JsonValue,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    if let Some(ref_str) = schema.get("$ref").and_then(|v| v.as_str()) {
        let (file_uri, fragment) = if ref_str.starts_with('#') {
            (current_uri.to_string(), Some(ref_str.to_string()))
        } else {
            let resolved = resolve_path_uri(current_uri, ref_str);
            let (f, frag) = split_uri_fragment(&resolved);
            (f.to_string(), frag.map(|s| s.to_string()))
        };

        let Some(target_root) = reg.get_by_uri(&file_uri) else {
            raise_error!(
                "ERR_SCHEMA_REF_NOT_FOUND",
                error = format!("Référence de schéma introuvable : {}", file_uri),
                context = json_value!({
                    "requested_uri": file_uri,
                    "action": "resolve_remote_ref",
                    "hint": "Assurez-vous que l'ontologie contenant cette URI a été chargée."
                })
            );
        };

        let target_schema = if let Some(frag) = fragment {
            let pointer = frag.replace('#', "");
            let Some(s) = target_root.pointer(&pointer) else {
                raise_error!(
                    "ERR_SCHEMA_POINTER_NOT_FOUND",
                    error = format!("Pointeur JSON '{}' introuvable dans {}", pointer, file_uri),
                    context = json_value!({
                        "pointer": pointer,
                        "target_file": file_uri,
                        "action": "resolve_json_pointer",
                        "hint": "Vérifiez que le chemin existe dans le document source. Les pointeurs sont sensibles à la casse."
                    })
                );
            };
            s
        } else {
            target_root
        };

        return validate_node(instance, target_schema, reg, &file_uri);
    }

    if let Some(t) = schema.get("type").and_then(|v| v.as_str()) {
        match t {
            "object" => {
                if instance.is_object() {
                    validate_object(instance, schema, reg, current_uri)?;
                } else {
                    raise_type_error("object", instance)?;
                }
            }
            "string" => {
                if instance.is_string() {
                    validate_string(instance, schema)?;
                } else {
                    raise_type_error("string", instance)?;
                }
            }
            "number" => {
                if instance.is_number() {
                    validate_number(instance, schema)?;
                } else {
                    raise_type_error("number", instance)?;
                }
            }
            "integer" => {
                if instance.is_i64() || instance.is_u64() {
                    validate_number(instance, schema)?;
                } else {
                    raise_type_error("integer", instance)?;
                }
            }
            "boolean" => {
                if !instance.is_boolean() {
                    raise_type_error("boolean", instance)?;
                }
            }
            "array" => {
                if instance.is_array() {
                    validate_array(instance, schema, reg, current_uri)?;
                } else {
                    raise_type_error("array", instance)?;
                }
            }
            "null" => {
                if !instance.is_null() {
                    raise_type_error("null", instance)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn raise_type_error(expected: &str, actual: &JsonValue) -> RaiseResult<()> {
    raise_error!(
        "ERR_VALIDATION_TYPE_MISMATCH",
        error = format!("Échec de conformité : type '{}' attendu.", expected),
        context = json_value!({
            "expected_type": expected,
            "actual_value_sample": format!("{:.50}", actual.to_string()),
            "action": "validate_primitive_type",
            "hint": format!("La donnée ne correspond pas à la définition du schéma (attendu: {}).", expected)
        })
    );
}

fn validate_object(
    instance: &JsonValue,
    schema: &JsonValue,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    let Some(obj) = instance.as_object() else {
        return Ok(());
    };

    if let Some(req) = schema.get("required").and_then(|v| v.as_array()) {
        for r in req {
            if let Some(key) = r.as_str() {
                if !obj.contains_key(key) {
                    raise_error!(
                        "ERR_VALIDATION_REQUIRED_FIELD_MISSING",
                        error = format!("Propriété obligatoire manquante : '{}'", key),
                        context = json_value!({
                            "missing_key": key,
                            "available_keys": obj.keys().collect::<Vec<_>>(),
                            "action": "validate_required_properties",
                            "hint": format!("L'objet doit contenir la clé '{}' pour être conforme au schéma.", key)
                        })
                    );
                }
            }
        }
    }

    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, sub_schema) in props {
            if let Some(val) = obj.get(key) {
                if let Err(e) = validate_node(val, sub_schema, reg, current_uri) {
                    raise_error!(
                        "ERR_VALIDATION_NESTED_PROPERTY_FAIL",
                        error = format!("Échec de validation sur la propriété '{}'", key),
                        context = json_value!({
                            "property_name": key,
                            "nested_error": e,
                            "action": "validate_object_properties",
                            "hint": "Une sous-propriété de cet objet ne respecte pas son schéma dédié."
                        })
                    );
                }
            }
        }
    }

    let mut compiled_patterns = Vec::new();
    if let Some(patterns) = schema.get("patternProperties").and_then(|v| v.as_object()) {
        for (pattern, sub_schema) in patterns {
            let re = match TextRegex::new(pattern) {
                Ok(r) => r,
                Err(e) => {
                    raise_error!(
                        "ERR_SCHEMA_INVALID_REGEX_PATTERN",
                        error = format!("Regex invalide dans 'patternProperties' : {}", pattern),
                        context = json_value!({
                            "invalid_pattern": pattern,
                            "regex_error": e.to_string(),
                            "action": "compile_pattern_properties",
                            "hint": "Vérifiez la syntaxe de votre expression régulière."
                        })
                    );
                }
            };

            for (key, val) in obj {
                if re.is_match(key) {
                    if let Err(e) = validate_node(val, sub_schema, reg, current_uri) {
                        raise_error!(
                            "ERR_VALIDATION_PATTERN_PROPERTY_FAIL",
                            error = format!("Échec de validation pour la clé dynamique '{}'", key),
                            context = json_value!({
                                "matched_key": key,
                                "applied_pattern": re.as_str(),
                                "nested_error": e,
                                "action": "validate_pattern_properties",
                                "hint": "La donnée associée à cette clé ne respecte pas le schéma imposé par le motif Regex."
                            })
                        );
                    }
                }
            }
            compiled_patterns.push(re);
        }
    }

    if let Some(ap) = schema.get("additionalProperties") {
        let is_allowed = if ap.is_boolean() {
            ap.as_bool().unwrap_or(true)
        } else {
            true
        };

        if !is_allowed {
            let defined_props: Vec<&String> = schema
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|m| m.keys().collect())
                .unwrap_or_default();

            for k in obj.keys() {
                let is_defined = defined_props.contains(&k);
                let matches_pattern = compiled_patterns.iter().any(|re| re.is_match(k));

                if !is_defined && !matches_pattern && k != "$schema" && k != "@context" {
                    raise_error!(
                        "ERR_VALIDATION_ADDITIONAL_PROPERTY_FORBIDDEN",
                        error = format!("Propriété non autorisée détectée : '{}'", k),
                        context = json_value!({
                            "forbidden_key": k,
                            "action": "validate_additional_properties",
                            "hint": "Ce schéma est clos (additionalProperties: false). Seules les propriétés explicitement définies sont acceptées."
                        })
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_string(instance: &JsonValue, schema: &JsonValue) -> RaiseResult<()> {
    let Some(s) = instance.as_str() else {
        return Ok(());
    };

    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        if s.chars().count() < min as usize {
            raise_error!(
                "ERR_VALIDATION_STRING_TOO_SHORT",
                error = format!("La chaîne est trop courte (minimum: {} caractères).", min),
                context = json_value!({ "actual_length": s.chars().count(), "min_length": min })
            );
        }
    }

    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        if s.chars().count() > max as usize {
            raise_error!(
                "ERR_VALIDATION_STRING_TOO_LONG",
                error = format!("La chaîne est trop longue (maximum: {} caractères).", max),
                context = json_value!({ "actual_length": s.chars().count(), "max_length": max })
            );
        }
    }

    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        let re = match TextRegex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                raise_error!(
                    "ERR_SCHEMA_INVALID_REGEX",
                    error = format!("Regex invalide dans le schéma : {}", pattern),
                    context = json_value!({ "pattern": pattern, "error": e.to_string() })
                );
            }
        };
        if !re.is_match(s) {
            raise_error!(
                "ERR_VALIDATION_PATTERN_MISMATCH",
                error = "Le format de la chaîne ne correspond pas au motif exigé.",
                context = json_value!({ "pattern": pattern, "value": s })
            );
        }
    }
    Ok(())
}

fn validate_number(instance: &JsonValue, schema: &JsonValue) -> RaiseResult<()> {
    let Some(n) = instance.as_f64() else {
        return Ok(());
    };

    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        if n < min {
            raise_error!(
                "ERR_VALIDATION_NUMBER_TOO_SMALL",
                error = format!("La valeur est inférieure au minimum autorisé ({}).", min),
                context = json_value!({ "actual_value": n, "minimum": min })
            );
        }
    }

    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        if n > max {
            raise_error!(
                "ERR_VALIDATION_NUMBER_TOO_LARGE",
                error = format!("La valeur est supérieure au maximum autorisé ({}).", max),
                context = json_value!({ "actual_value": n, "maximum": max })
            );
        }
    }
    Ok(())
}

fn validate_array(
    instance: &JsonValue,
    schema: &JsonValue,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    let Some(arr) = instance.as_array() else {
        return Ok(());
    };

    if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
        if arr.len() < min as usize {
            raise_error!(
                "ERR_VALIDATION_ARRAY_TOO_SMALL",
                error = format!(
                    "Le tableau contient trop peu d'éléments (minimum: {}).",
                    min
                ),
                context = json_value!({ "actual_length": arr.len(), "min_items": min })
            );
        }
    }

    if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
        if arr.len() > max as usize {
            raise_error!(
                "ERR_VALIDATION_ARRAY_TOO_LARGE",
                error = format!("Le tableau contient trop d'éléments (maximum: {}).", max),
                context = json_value!({ "actual_length": arr.len(), "max_items": max })
            );
        }
    }

    if let Some(items_schema) = schema.get("items") {
        if items_schema.is_object() {
            for (index, item) in arr.iter().enumerate() {
                if let Err(e) = validate_node(item, items_schema, reg, current_uri) {
                    raise_error!(
                        "ERR_VALIDATION_ARRAY_ITEM_FAIL",
                        error = format!("Échec de validation à l'index [{}] du tableau", index),
                        context = json_value!({ "index": index, "nested_error": e })
                    );
                }
            }
        }
    }
    Ok(())
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
        normalized.to_string_lossy().replace('\\', "/")
    )
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            fs::Component::CurDir => {}
            fs::Component::ParentDir => {
                components.pop();
            }
            fs::Component::Normal(c) => components.push(c),
            fs::Component::RootDir | fs::Component::Prefix(_) => {}
        }
    }
    let mut result = PathBuf::new();
    for c in components {
        result.push(c);
    }
    result
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_validator(schema: JsonValue) -> SchemaValidator {
        let mut reg = SchemaRegistry::new();
        reg.register("db://test/schema".to_string(), schema);
        SchemaValidator::compile_with_registry("db://test/schema", &reg).unwrap()
    }

    #[test]
    fn test_string_constraints() {
        let v = setup_validator(json_value!({
            "type": "string",
            "minLength": 3,
            "maxLength": 5,
            "pattern": "^[A-Z]+$"
        }));

        assert!(v.validate(&json_value!("TEST")).is_ok());
        assert!(v.validate(&json_value!("TE")).is_err());
        assert!(v.validate(&json_value!("TESTING")).is_err());
        assert!(v.validate(&json_value!("test")).is_err());
    }

    #[test]
    fn test_number_constraints() {
        let v = setup_validator(json_value!({
            "type": "number",
            "minimum": 10.5,
            "maximum": 20.0
        }));

        assert!(v.validate(&json_value!(15)).is_ok());
        assert!(v.validate(&json_value!(10.5)).is_ok());
        assert!(v.validate(&json_value!(5)).is_err());
        assert!(v.validate(&json_value!(21.5)).is_err());
    }

    #[test]
    fn test_apply_defaults_basic() {
        let v = setup_validator(json_value!({
            "type": "object",
            "properties": {
                "active": { "type": "boolean", "default": true },
                "version": { "type": "integer", "default": 1 }
            }
        }));

        let mut data = json_value!({});
        v.compute_then_validate(&mut data)
            .expect("L'hydratation doit réussir");

        assert_eq!(data["active"], true);
        assert_eq!(data["version"], 1);
    }

    #[test]
    fn test_dual_schema_x_compute() {
        let mut reg = SchemaRegistry::new();

        reg.register("db://test/v1".to_string(), json_value!({
            "type": "object",
            "properties": {
                "Id": { "type": "string", "x_compute": { "update": "if_missing", "plan": { "op": "uuid_v4" } } }
            }
        }));

        reg.register("db://test/v2".to_string(), json_value!({
            "type": "object",
            "properties": {
                "_id": { "type": "string", "x_compute": { "update": "if_missing", "plan": { "op": "uuid_v4" } } }
            }
        }));

        let v1 = SchemaValidator::compile_with_registry("db://test/v1", &reg).unwrap();
        let v2 = SchemaValidator::compile_with_registry("db://test/v2", &reg).unwrap();

        let mut data_v1 = json_value!({});
        let mut data_v2 = json_value!({});

        v1.compute_then_validate(&mut data_v1).unwrap();
        v2.compute_then_validate(&mut data_v2).unwrap();

        assert!(
            data_v1.get("Id").is_some(),
            "Le schéma v1 n'a pas injecté 'Id'"
        );
        assert!(
            data_v2.get("_id").is_some(),
            "Le schéma v2 n'a pas injecté '_id'"
        );
    }
}
