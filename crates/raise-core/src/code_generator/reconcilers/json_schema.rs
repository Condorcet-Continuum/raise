// FICHIER : crates/raise-core/src/code_generator/reconcilers/json_schema.rs

use crate::code_generator::models::{CompositionStrategy, JsonSchemaElement, SchemaType};
use crate::utils::prelude::*;

pub struct Reconciler;

impl Reconciler {
    /// 🚀 Lecture physique depuis le disque (Bottom-Up)
    pub async fn parse_from_file(
        path: &Path,
        module_id: String,
    ) -> RaiseResult<Vec<JsonSchemaElement>> {
        let content = match fs::read_to_string_async(path).await {
            Ok(c) => c,
            Err(e) => raise_error!(
                "ERR_SYSTEM_IO",
                error = e,
                context = json_value!({ "action": "read_schema_async", "path": path.display().to_string() })
            ),
        };

        // Déduction formelle du handle à partir du nom de fichier
        // (ex: blender_input.schema.json -> schema_blender_input)
        let file_stem = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .replace(".schema.json", "")
            .replace(".json", "")
            .replace("-", "_");

        let handle = format!("schema_{}", file_stem);

        Self::parse_content(&content, module_id, handle)
    }

    /// 🧠 Extraction Sémantique et Validation Stricte
    pub fn parse_content(
        content: &str,
        module_id: String,
        handle: String,
    ) -> RaiseResult<Vec<JsonSchemaElement>> {
        let mut parsed: JsonValue = match json::deserialize_from_str(content) {
            Ok(v) => v,
            Err(e) => raise_error!(
                "ERR_JSON_SCHEMA_PARSE",
                error = format!("JSON invalide : {}", e),
                context = json_value!({ "handle": handle })
            ),
        };

        // 1. Déduction stricte de la norme (Draft)
        let draft = match parsed.get("$schema").and_then(|s| s.as_str()) {
            Some(uri) if uri.contains("2020-12") => "2020-12".to_string(),
            Some(uri) if uri.contains("2019-09") => "2019-09".to_string(),
            Some(uri) if uri.contains("draft-07") => "7".to_string(),
            Some(uri) if uri.contains("draft-04") => "4".to_string(),
            _ => "2020-12".to_string(), // Convention RAISE par défaut
        };

        // 2. Identification Typologique
        let schema_type = match parsed.get("type").and_then(|t| t.as_str()) {
            Some("object") => SchemaType::Object,
            Some("array") => SchemaType::Array,
            Some("string") => SchemaType::String,
            Some("number") | Some("integer") => SchemaType::Number,
            Some("boolean") => SchemaType::Boolean,
            Some("null") => SchemaType::Null,
            _ => {
                // Si type absent, ou si on a un tableau de types
                if parsed.get("type").and_then(|t| t.as_array()).is_some() {
                    SchemaType::Multi
                } else {
                    SchemaType::Object // Type implicite majoritaire
                }
            }
        };

        // 3. Détection de la Composition (Héritage MBSE)
        let composition_strategy = if parsed.get("allOf").is_some() {
            CompositionStrategy::AllOf
        } else if parsed.get("anyOf").is_some() {
            CompositionStrategy::AnyOf
        } else if parsed.get("oneOf").is_some() {
            CompositionStrategy::OneOf
        } else if parsed.get("not").is_some() {
            CompositionStrategy::Not
        } else {
            CompositionStrategy::None
        };

        // 4. Extraction du Call Graph (Dépendances Externes)
        let mut external_dependencies = Vec::new();
        Self::extract_refs(&parsed, &mut external_dependencies);
        external_dependencies.sort();
        external_dependencies.dedup();

        // 5. Traçabilité URI et Nettoyage
        let mut metadata = UnorderedMap::new();
        if let Some(id_val) = parsed.get("$id").and_then(|id| id.as_str()) {
            metadata.insert("schema_uri".to_string(), id_val.to_string());
        }

        // Nettoyage des attributs managés par le Jumeau Numérique (injectés au tissage)
        if let Some(obj) = parsed.as_object_mut() {
            obj.remove("$schema");
            obj.remove("$id");
        }

        let element = JsonSchemaElement {
            module_id: Some(module_id),
            parent_id: None,
            handle,
            draft,
            schema_type,
            composition_strategy,
            content: parsed,
            external_dependencies,
            target_binding: None,
            validation_config: None,
            metadata,
        };

        Ok(vec![element])
    }

    /// 🕸️ Traverse l'AST JSON récursivement pour capturer toutes les dépendances URIs
    fn extract_refs(val: &JsonValue, refs: &mut Vec<String>) {
        match val {
            JsonValue::Object(obj) => {
                for (k, v) in obj {
                    if k == "$ref" {
                        if let Some(r) = v.as_str() {
                            refs.push(r.to_string());
                        }
                    } else {
                        Self::extract_refs(v, refs);
                    }
                }
            }
            JsonValue::Array(arr) => {
                for v in arr {
                    Self::extract_refs(v, refs);
                }
            }
            _ => {}
        }
    }
}

// =========================================================================
// TESTS UNITAIRES (Robustesse Zéro Dette)
// =========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_json_schema_basic() -> RaiseResult<()> {
        let content = r#"{
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "$id": "db://test/my_schema",
            "type": "object",
            "properties": {
                "name": { "type": "string" }
            }
        }"#;

        let elements =
            Reconciler::parse_content(content, "mod_123".to_string(), "schema_test".to_string())?;

        assert_eq!(elements.len(), 1);
        let el = &elements[0];

        assert_eq!(el.handle, "schema_test");
        assert_eq!(el.draft, "2020-12");
        assert_eq!(el.schema_type, SchemaType::Object);
        assert_eq!(
            el.metadata
                .get("schema_uri")
                .expect("L'URI doit être extraite"),
            "db://test/my_schema"
        );

        // Vérifie le nettoyage des champs redondants
        assert!(el.content.get("$schema").is_none());
        assert!(el.content.get("$id").is_none());

        Ok(())
    }

    #[test]
    fn test_parse_json_schema_composition_and_refs() -> RaiseResult<()> {
        let content = r#"{
            "allOf": [
                { "$ref": "db://test/base" },
                {
                    "type": "object",
                    "properties": {
                        "child": { "$ref": "db://test/child" }
                    }
                }
            ]
        }"#;

        let elements =
            Reconciler::parse_content(content, "mod_123".to_string(), "schema_comp".to_string())?;
        let el = &elements[0];

        assert_eq!(el.composition_strategy, CompositionStrategy::AllOf);
        assert_eq!(el.external_dependencies.len(), 2);
        assert!(el
            .external_dependencies
            .contains(&"db://test/base".to_string()));
        assert!(el
            .external_dependencies
            .contains(&"db://test/child".to_string()));

        Ok(())
    }

    #[test]
    fn test_parse_invalid_json_fails_fast() {
        let content = r#"{ "type": "object", broken_json }"#;

        let result =
            Reconciler::parse_content(content, "mod_123".to_string(), "schema_fail".to_string());

        assert!(
            result.is_err(),
            "La validation stricte doit bloquer un JSON corrompu"
        );

        if let Err(crate::utils::core::error::AppError::Structured(err)) = result {
            assert_eq!(err.code, "ERR_JSON_SCHEMA_PARSE");
        } else {
            panic!("Code d'erreur inattendu.");
        }
    }
}
