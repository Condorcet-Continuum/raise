// FICHIER : src-tauri/src/json_db/schema/validator.rs

use super::registry::SchemaRegistry;

use crate::utils::io::{Component, Path, PathBuf};
use crate::utils::prelude::*;
use crate::utils::Regex;

#[derive(Debug, Clone)]
pub struct SchemaValidator {
    root_uri: String,
    schema: Value,
    reg: SchemaRegistry,
}

impl SchemaValidator {
    pub fn compile_with_registry(root_uri: &str, reg: &SchemaRegistry) -> RaiseResult<Self> {
        let schema = reg.get_by_uri(root_uri).cloned().ok_or_else(|| {
            AppError::NotFound(format!("Schema not found in registry: {}", root_uri))
        })?;
        Ok(Self {
            root_uri: root_uri.to_string(),
            schema,
            reg: reg.clone(),
        })
    }

    pub fn compute_then_validate(&self, instance: &mut Value) -> RaiseResult<()> {
        // L'ancien moteur "x_compute" est désactivé.
        // Les calculs sont désormais gérés par le Rules Engine dans manager.rs avant d'arriver ici.
        self.validate(instance)
    }

    pub fn validate(&self, instance: &Value) -> RaiseResult<()> {
        validate_node(instance, &self.schema, &self.reg, &self.root_uri)
    }
}

fn validate_node(
    instance: &Value,
    schema: &Value,
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

        let target_root = reg
            .get_by_uri(&file_uri)
            .ok_or_else(|| AppError::NotFound(format!("Ref schema not found: {}", file_uri)))?;

        let target_schema = if let Some(frag) = fragment {
            let pointer = frag.replace('#', "");
            target_root.pointer(&pointer).ok_or_else(|| {
                AppError::NotFound(format!("Pointer {} not found in {}", pointer, file_uri))
            })?
        } else {
            target_root
        };

        return validate_node(instance, target_schema, reg, &file_uri);
    }

    if let Some(t) = schema.get("type").and_then(|v| v.as_str()) {
        match t {
            "object" => {
                if !instance.is_object() {
                    return Err(AppError::Database(format!(
                        "Expected object, got {:?}",
                        instance
                    )));
                }
                validate_object(instance, schema, reg, current_uri)?;
            }
            "string" => {
                if !instance.is_string() {
                    return Err(AppError::Validation("Expected string".to_string()));
                }
            }
            "number" => {
                if !instance.is_number() {
                    return Err(AppError::Validation("Expected number".to_string()));
                }
            }
            "integer" => {
                if !instance.is_i64() && !instance.is_u64() {
                    return Err(AppError::Validation("Expected integer".to_string()));
                }
            }
            "boolean" => {
                if !instance.is_boolean() {
                    return Err(AppError::Validation("Expected boolean".to_string()));
                }
            }
            "array" => {
                if !instance.is_array() {
                    return Err(AppError::Validation("Expected array".to_string()));
                }
            }
            "null" => {
                if !instance.is_null() {
                    return Err(AppError::Validation("Expected null".to_string()));
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn validate_object(
    instance: &Value,
    schema: &Value,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    let obj = instance.as_object().unwrap();

    // 1. Required
    if let Some(req) = schema.get("required").and_then(|v| v.as_array()) {
        for r in req {
            if let Some(key) = r.as_str() {
                if !obj.contains_key(key) {
                    return Err(AppError::Validation(format!(
                        "Missing required property: {}",
                        key
                    )));
                }
            }
        }
    }

    // 2. Properties
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, sub_schema) in props {
            if let Some(val) = obj.get(key) {
                validate_node(val, sub_schema, reg, current_uri)
                    .map_err(|e| AppError::Validation(format!("Property '{}': {}", key, e)))?;
            }
        }
    }

    // 3. Pattern Properties (CORRECTION : Ajout du support)
    let mut compiled_patterns = Vec::new();
    if let Some(patterns) = schema.get("patternProperties").and_then(|v| v.as_object()) {
        for (pattern, sub_schema) in patterns {
            // On compile le regex
            let re = Regex::new(pattern).map_err(|e| {
                AppError::Validation(format!(
                    "Invalid regex in patternProperties '{}': {}",
                    pattern, e
                ))
            })?;

            // On valide toutes les clés qui matchent
            for (key, val) in obj {
                if re.is_match(key) {
                    validate_node(val, sub_schema, reg, current_uri).map_err(|e| {
                        AppError::Validation(format!("Pattern property '{}': {}", key, e))
                    })?;
                }
            }
            compiled_patterns.push(re);
        }
    }

    // 4. Additional Properties
    if let Some(ap) = schema.get("additionalProperties") {
        // Si additionalProperties est false
        if ap.is_boolean() && !ap.as_bool().unwrap() {
            let defined_props: Vec<&String> = schema
                .get("properties")
                .and_then(|v| v.as_object())
                .map(|m| m.keys().collect())
                .unwrap_or_default();

            for k in obj.keys() {
                // Est-ce une propriété définie explicitement ?
                let is_defined = defined_props.contains(&k);

                // Est-ce une propriété correspondant à un pattern ?
                let matches_pattern = compiled_patterns.iter().any(|re| re.is_match(k));

                // Si ni l'un ni l'autre (et pas $schema/id qui sont souvent implicites ou injectés)
                // Note: On tolère $schema et id s'ils sont injectés par le système, mais idéalement ils devraient être dans le schéma.
                if !is_defined && !matches_pattern && k != "$schema" {
                    return Err(AppError::Validation(format!(
                        "Additional property not allowed: {}",
                        k
                    )));
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

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::json::json;

    #[test]
    fn test_simple_validation() {
        let mut reg = SchemaRegistry::new();
        let schema = json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": { "type": "string" },
                "age": { "type": "integer" }
            }
        });
        reg.register("db://test/schema".to_string(), schema);

        let validator = SchemaValidator::compile_with_registry("db://test/schema", &reg).unwrap();

        // Valid
        assert!(validator
            .validate(&json!({"name": "Alice", "age": 30}))
            .is_ok());

        // Invalid (missing required)
        assert!(validator.validate(&json!({"age": 30})).is_err());

        // Invalid (wrong type)
        assert!(validator
            .validate(&json!({"name": "Alice", "age": "trente"}))
            .is_err());
    }

    #[test]
    fn test_pattern_properties() {
        let mut reg = SchemaRegistry::new();
        let schema = json!({
            "type": "object",
            "patternProperties": {
                "^x_": { "type": "string" }
            },
            "additionalProperties": false
        });
        reg.register("db://test/pattern".to_string(), schema);
        let v = SchemaValidator::compile_with_registry("db://test/pattern", &reg).unwrap();

        assert!(v.validate(&json!({"x_factor": "yes"})).is_ok());
        assert!(v.validate(&json!({"y_factor": "no"})).is_err()); // Forbidden by additionalProperties: false
    }

    #[test]
    fn test_resolve_path() {
        let base = "db://space/db/schemas/v1/folder/file.json";
        let target = "../other/ref.json";
        let res = resolve_path_uri(base, target);
        assert_eq!(res, "db://space/db/schemas/v1/other/ref.json");
    }
}
