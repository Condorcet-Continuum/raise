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
        };

        // 2. Chemin nominal (Happy Path)
        Ok(Self {
            root_uri: root_uri.to_string(),
            schema,
            reg: reg.clone(),
        })
    }

    pub fn compute_then_validate(&self, instance: &mut Value) -> RaiseResult<()> {
        apply_defaults(instance, &self.schema, &self.reg, &self.root_uri)?;
        self.validate(instance)
    }

    pub fn validate(&self, instance: &Value) -> RaiseResult<()> {
        validate_node(instance, &self.schema, &self.reg, &self.root_uri)
    }
}

fn resolve_schema_node<'a>(
    schema: &'a Value,
    reg: &'a SchemaRegistry,
    current_uri: &str,
) -> &'a Value {
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

/// 🎯 Hydratation récursive des valeurs par défaut
fn apply_defaults(
    instance: &mut Value,
    schema: &Value,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    // 1. Résolution des références ($ref) pour trouver les defaults distants
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

    // 🎯 NOUVEAU : Traitement du mot-clé 'allOf' (Héritage)
    // C'est ce qui permet d'importer _id et _created_at depuis base.schema.json
    if let Some(all_of) = schema.get("allOf").and_then(|v| v.as_array()) {
        for sub_schema in all_of {
            apply_defaults(instance, sub_schema, reg, current_uri)?;
        }
    }

    // 2. Injection dans les Objets
    if let Some(obj) = instance.as_object_mut() {
        if let Some(props) = schema.get("properties").and_then(|v| v.as_object()) {
            for (key, sub_schema) in props {
                // 🎯 On résout le $ref pour lire les vraies instructions
                let resolved_schema = resolve_schema_node(sub_schema, reg, current_uri);

                // Petite optimisation idiomatique Rust (évite le unwrap)
                let is_missing = obj.get(key).is_none_or(|v| v.is_null());

                // A. Application du 'default' standard
                if is_missing {
                    // ✅ Priorité au default local avant le distant !
                    let default_val = sub_schema
                        .get("default")
                        .or_else(|| resolved_schema.get("default"));

                    if let Some(val) = default_val {
                        obj.insert(key.clone(), val.clone());
                    }
                }

                // B. EXÉCUTION DE 'x_compute'
                // ✅ Priorité au x_compute local
                let compute_node = sub_schema
                    .get("x_compute")
                    .or_else(|| resolved_schema.get("x_compute"))
                    .and_then(|v| v.as_object());

                if let Some(compute) = compute_node {
                    let update_strategy = compute
                        .get("update")
                        .and_then(|v| v.as_str())
                        .unwrap_or("if_missing");

                    // On recalcule 'is_missing' au cas où le 'default' juste au-dessus aurait injecté une valeur
                    let is_currently_missing = obj.get(key).is_none_or(|v| v.is_null());

                    let should_compute = match update_strategy {
                        "always" => true,
                        "if_missing" => is_currently_missing,
                        _ => false,
                    };

                    if should_compute {
                        if let Some(plan) = compute.get("plan").and_then(|v| v.as_object()) {
                            if let Some(op) = plan.get("op").and_then(|v| v.as_str()) {
                                let computed_val = match op {
                                    "uuid_v4" => Value::String(Uuid::new_v4().to_string()),
                                    "now_rfc3339" => Value::String(Utc::now().to_rfc3339()),
                                    "const" => plan.get("value").cloned().unwrap_or(Value::Null),
                                    _ => Value::Null,
                                };

                                if !computed_val.is_null() {
                                    obj.insert(key.clone(), computed_val);
                                }
                            }
                        }
                    }
                }

                // Récursion sur la valeur (pré-existante ou fraîchement calculée)
                if let Some(val) = obj.get_mut(key) {
                    apply_defaults(val, sub_schema, reg, current_uri)?;
                }
            }
        }
    }

    // 3. Injection dans les Tableaux
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
                validate_string(instance, schema)?;
            }
            "number" => {
                if !instance.is_number() {
                    raise_type_error("number", instance)?;
                }
                validate_number(instance, schema)?;
            }
            "integer" => {
                if !instance.is_i64() && !instance.is_u64() {
                    raise_type_error("integer", instance)?;
                }
                validate_number(instance, schema)?;
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
                validate_array(instance, schema, reg, current_uri)?;
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
fn validate_string(instance: &Value, schema: &Value) -> RaiseResult<()> {
    let s = instance.as_str().unwrap();

    if let Some(min) = schema.get("minLength").and_then(|v| v.as_u64()) {
        if s.chars().count() < min as usize {
            raise_error!(
                "ERR_VALIDATION_STRING_TOO_SHORT",
                error = format!("La chaîne est trop courte (minimum: {} caractères).", min),
                context = json!({ "actual_length": s.chars().count(), "min_length": min })
            );
        }
    }

    if let Some(max) = schema.get("maxLength").and_then(|v| v.as_u64()) {
        if s.chars().count() > max as usize {
            raise_error!(
                "ERR_VALIDATION_STRING_TOO_LONG",
                error = format!("La chaîne est trop longue (maximum: {} caractères).", max),
                context = json!({ "actual_length": s.chars().count(), "max_length": max })
            );
        }
    }

    if let Some(pattern) = schema.get("pattern").and_then(|v| v.as_str()) {
        let re = match Regex::new(pattern) {
            Ok(r) => r,
            Err(e) => {
                raise_error!(
                    "ERR_SCHEMA_INVALID_REGEX",
                    error = format!("Regex invalide dans le schéma : {}", pattern),
                    context = json!({ "pattern": pattern, "error": e.to_string() })
                );
            }
        };
        if !re.is_match(s) {
            raise_error!(
                "ERR_VALIDATION_PATTERN_MISMATCH",
                error = "Le format de la chaîne ne correspond pas au motif exigé.",
                context = json!({ "pattern": pattern, "value": s })
            );
        }
    }
    Ok(())
}

fn validate_number(instance: &Value, schema: &Value) -> RaiseResult<()> {
    // as_f64() gère proprement les entiers et les flottants pour la comparaison
    let n = instance.as_f64().unwrap();

    if let Some(min) = schema.get("minimum").and_then(|v| v.as_f64()) {
        if n < min {
            raise_error!(
                "ERR_VALIDATION_NUMBER_TOO_SMALL",
                error = format!("La valeur est inférieure au minimum autorisé ({}).", min),
                context = json!({ "actual_value": n, "minimum": min })
            );
        }
    }

    if let Some(max) = schema.get("maximum").and_then(|v| v.as_f64()) {
        if n > max {
            raise_error!(
                "ERR_VALIDATION_NUMBER_TOO_LARGE",
                error = format!("La valeur est supérieure au maximum autorisé ({}).", max),
                context = json!({ "actual_value": n, "maximum": max })
            );
        }
    }
    Ok(())
}

fn validate_array(
    instance: &Value,
    schema: &Value,
    reg: &SchemaRegistry,
    current_uri: &str,
) -> RaiseResult<()> {
    let arr = instance.as_array().unwrap();

    if let Some(min) = schema.get("minItems").and_then(|v| v.as_u64()) {
        if arr.len() < min as usize {
            raise_error!(
                "ERR_VALIDATION_ARRAY_TOO_SMALL",
                error = format!(
                    "Le tableau contient trop peu d'éléments (minimum: {}).",
                    min
                ),
                context = json!({ "actual_length": arr.len(), "min_items": min })
            );
        }
    }

    if let Some(max) = schema.get("maxItems").and_then(|v| v.as_u64()) {
        if arr.len() > max as usize {
            raise_error!(
                "ERR_VALIDATION_ARRAY_TOO_LARGE",
                error = format!("Le tableau contient trop d'éléments (maximum: {}).", max),
                context = json!({ "actual_length": arr.len(), "max_items": max })
            );
        }
    }

    // Validation récursive de chaque élément du tableau
    if let Some(items_schema) = schema.get("items") {
        if items_schema.is_object() {
            for (index, item) in arr.iter().enumerate() {
                if let Err(e) = validate_node(item, items_schema, reg, current_uri) {
                    raise_error!(
                        "ERR_VALIDATION_ARRAY_ITEM_FAIL",
                        error = format!("Échec de validation à l'index [{}] du tableau", index),
                        context = json!({ "index": index, "nested_error": e })
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

    fn setup_validator(schema: Value) -> SchemaValidator {
        let mut reg = SchemaRegistry::new();
        reg.register("db://test/schema".to_string(), schema);
        SchemaValidator::compile_with_registry("db://test/schema", &reg).unwrap()
    }

    #[test]
    fn test_string_constraints() {
        let v = setup_validator(json!({
            "type": "string",
            "minLength": 3,
            "maxLength": 5,
            "pattern": "^[A-Z]+$"
        }));

        assert!(v.validate(&json!("TEST")).is_ok()); // 4 chars, Majuscules
        assert!(v.validate(&json!("TE")).is_err()); // Trop court
        assert!(v.validate(&json!("TESTING")).is_err()); // Trop long
        assert!(v.validate(&json!("test")).is_err()); // Mauvais pattern (minuscules)
    }

    #[test]
    fn test_number_constraints() {
        let v = setup_validator(json!({
            "type": "number",
            "minimum": 10.5,
            "maximum": 20.0
        }));

        assert!(v.validate(&json!(15)).is_ok());
        assert!(v.validate(&json!(10.5)).is_ok());
        assert!(v.validate(&json!(5)).is_err()); // Trop petit
        assert!(v.validate(&json!(21.5)).is_err()); // Trop grand
    }

    #[test]
    fn test_array_constraints() {
        let v = setup_validator(json!({
            "type": "array",
            "minItems": 1,
            "maxItems": 3,
            "items": { "type": "string" }
        }));

        assert!(v.validate(&json!(["a", "b"])).is_ok()); // Valide (taille 2, full strings)
        assert!(v.validate(&json!([])).is_err()); // Trop petit
        assert!(v.validate(&json!(["a", "b", "c", "d"])).is_err()); // Trop grand
        assert!(v.validate(&json!(["a", 42])).is_err()); // 42 n'est pas une string
    }
    #[test]
    fn test_apply_defaults_basic() {
        let v = setup_validator(json!({
            "type": "object",
            "properties": {
                "active": { "type": "boolean", "default": true },
                "version": { "type": "integer", "default": 1 }
            }
        }));

        let mut data = json!({});
        v.compute_then_validate(&mut data)
            .expect("L'hydratation doit réussir");

        assert_eq!(data["active"], true);
        assert_eq!(data["version"], 1);
    }

    #[test]
    fn test_apply_defaults_recursive() {
        let v = setup_validator(json!({
            "type": "object",
            "properties": {
                "settings": {
                    "type": "object",
                    "default": {},
                    "properties": {
                        "theme": { "type": "string", "default": "dark" }
                    }
                }
            }
        }));

        let mut data = json!({});
        v.compute_then_validate(&mut data)
            .expect("L'hydratation récursive doit réussir");

        // Vérifie que 'settings' a été créé ET que 'theme' y a été injecté
        assert_eq!(data["settings"]["theme"], "dark");
    }

    #[test]
    fn test_required_with_default_interplay() {
        let v = setup_validator(json!({
            "type": "object",
            "properties": {
                "id": { "type": "string" },
                "status": { "type": "string", "default": "pending" }
            },
            "required": ["id", "status"]
        }));

        // Scénario 1: id présent, status manquant -> Succès (status injecté)
        let mut data1 = json!({ "id": "u1" });
        v.compute_then_validate(&mut data1)
            .expect("Doit passer car status est injecté");
        assert_eq!(data1["status"], "pending");

        // Scénario 2: tout manquant -> Échec (id n'a pas de default)
        let mut data2 = json!({});
        let res = v.compute_then_validate(&mut data2);
        assert!(
            res.is_err(),
            "Doit échouer car 'id' est requis et sans default"
        );

        let err_msg = res.unwrap_err().to_string();
        assert!(err_msg.contains("ERR_VALIDATION_REQUIRED_FIELD_MISSING"));
    }

    #[test]
    fn test_apply_defaults_in_arrays() {
        let v = setup_validator(json!({
            "type": "array",
            "items": {
                "type": "object",
                "properties": {
                    "role": { "type": "string", "default": "user" }
                }
            }
        }));

        let mut data = json!([{}, {"role": "admin"}]);
        v.compute_then_validate(&mut data).unwrap();

        assert_eq!(data[0]["role"], "user"); // Injecté
        assert_eq!(data[1]["role"], "admin"); // Préservé
    }
}
