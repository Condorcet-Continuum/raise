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
        // 1. Garde let-else avec diagnostic structuré
        let Some(schema) = reg.get_by_uri(root_uri).cloned() else {
            raise_error!(
                "ERR_SCHEMA_NOT_IN_REGISTRY",
                error = format!("Le schéma sémantique est introuvable : {}", root_uri),
                context = json!({
                    "root_uri": root_uri,
                    "action": "resolve_schema_from_registry",
                    "hint": "Vérifiez que l'ontologie a été correctement chargée."
                })
            ); // La macro fait un 'return', donc on ne sort jamais du 'else' ici.
        }; // <--- L'accolade manquante était ici !

        // 2. Chemin nominal (Happy Path)
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

        let Some(target_root) = reg.get_by_uri(&file_uri) else {
            raise_error!(
                "ERR_SCHEMA_REF_NOT_FOUND",
                error = format!("Référence de schéma introuvable : {}", file_uri),
                context = json!({
                    "requested_uri": file_uri,
                    "action": "resolve_remote_ref",
                    "hint": "Assurez-vous que l'ontologie contenant cette URI a été chargée via 'load_layer_from_file' avant la validation."
                })
            );
        };
        let target_schema = if let Some(frag) = fragment {
            let pointer = frag.replace('#', "");

            match target_root.pointer(&pointer) {
                Some(s) => s,
                None => {
                    raise_error!(
                        "ERR_SCHEMA_POINTER_NOT_FOUND",
                        error =
                            format!("Pointeur JSON '{}' introuvable dans {}", pointer, file_uri),
                        context = json!({
                            "pointer": pointer,
                            "target_file": file_uri,
                            "action": "resolve_json_pointer",
                            "hint": "Vérifiez que le chemin existe dans le document source. Les pointeurs sont sensibles à la casse."
                        })
                    );
                }
            }
        } else {
            target_root
        };

        return validate_node(instance, target_schema, reg, &file_uri);
    }

    if let Some(t) = schema.get("type").and_then(|v| v.as_str()) {
        match t {
            "object" => {
                if !instance.is_object() {
                    raise_type_error("object", instance)?;
                }
                validate_object(instance, schema, reg, current_uri)?;
            }
            "string" => {
                if !instance.is_string() {
                    raise_type_error("string", instance)?;
                }
            }
            "number" => {
                if !instance.is_number() {
                    raise_type_error("number", instance)?;
                }
            }
            "integer" => {
                if !instance.is_i64() && !instance.is_u64() {
                    raise_type_error("integer", instance)?;
                }
            }
            "boolean" => {
                if !instance.is_boolean() {
                    raise_type_error("boolean", instance)?;
                }
            }
            "array" => {
                if !instance.is_array() {
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

    // Fonction utilitaire de diagnostic (Standard RAISE V1.3)
    fn raise_type_error(expected: &str, actual: &Value) -> RaiseResult<()> {
        raise_error!(
            "ERR_VALIDATION_TYPE_MISMATCH",
            error = format!("Échec de conformité : type '{}' attendu.", expected),
            context = json!({
                "expected_type": expected,
                "actual_value_sample": format!("{:.50}", actual.to_string()),
                "action": "validate_primitive_type",
                "hint": format!("La donnée ne correspond pas à la définition du schéma (attendu: {}).", expected)
            })
        );
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
                    raise_error!(
                        "ERR_VALIDATION_REQUIRED_FIELD_MISSING",
                        error = format!("Propriété obligatoire manquante : '{}'", key),
                        context = json!({
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

    // 2. Properties
    if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
        for (key, sub_schema) in props {
            if let Some(val) = obj.get(key) {
                if let Err(e) = validate_node(val, sub_schema, reg, current_uri) {
                    raise_error!(
                        "ERR_VALIDATION_NESTED_PROPERTY_FAIL",
                        error = format!("Échec de validation sur la propriété '{}'", key),
                        context = json!({
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

    // 3. Pattern Properties (CORRECTION : Ajout du support)
    let mut compiled_patterns = Vec::new();
    if let Some(patterns) = schema.get("patternProperties").and_then(|v| v.as_object()) {
        for (pattern, sub_schema) in patterns {
            // On compile le regex
            let re = match Regex::new(pattern) {
                Ok(r) => r,
                Err(e) => {
                    raise_error!(
                        "ERR_SCHEMA_INVALID_REGEX_PATTERN",
                        error = format!("Regex invalide dans 'patternProperties' : {}", pattern),
                        context = json!({
                            "invalid_pattern": pattern,
                            "regex_error": e.to_string(),
                            "action": "compile_pattern_properties",
                            "hint": "Vérifiez la syntaxe de votre expression régulière. Les Regex doivent respecter le standard Rust (crate regex)."
                        })
                    );
                }
            };

            // On valide toutes les clés qui matchent
            for (key, val) in obj {
                if re.is_match(key) {
                    if let Err(e) = validate_node(val, sub_schema, reg, current_uri) {
                        raise_error!(
                            "ERR_VALIDATION_PATTERN_PROPERTY_FAIL",
                            error = format!("Échec de validation pour la clé dynamique '{}'", key),
                            context = json!({
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
                // MIGRATION V1.3 : Validation des propriétés additionnelles (Schéma clos)
                if !is_defined && !matches_pattern && k != "$schema" {
                    raise_error!(
                        "ERR_VALIDATION_ADDITIONAL_PROPERTY_FORBIDDEN",
                        error = format!("Propriété non autorisée détectée : '{}'", k),
                        context = json!({
                            "forbidden_key": k,
                            "action": "validate_additional_properties",
                            "hint": "Ce schéma est clos (additionalProperties: false). Seules les propriétés définies explicitement ou correspondant à un motif (pattern) sont acceptées."
                        })
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
