// FICHIER : src-tauri/src/rules_engine/ast.rs

use crate::utils::prelude::*;

/// Représentation en mémoire d'une règle définie dans 'quality-rule.schema.json'.
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
pub struct Rule {
    /// 🆔 L'UUID technique (nom du fichier).
    /// Optionnel car généré par le Manager lors du premier insert.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub _id: Option<String>,

    /// 🏷️ L'identité métier stable (ex: "rule-auto-hd").
    /// Utilisé comme clé dans le registre 'rules: {}' de l'index.
    #[serde(alias = "id")]
    pub handle: String,

    #[serde(rename = "target_path", alias = "target")]
    pub target: String,

    pub expr: Expr,

    #[serde(default)]
    pub description: Option<String>,

    #[serde(default)]
    pub severity: Option<String>,
}

/// Arbre Syntaxique Abstrait (AST) complet.
/// Union des fonctionnalités Legacy (Analyzer) et Modernes (Validator).
#[derive(Debug, Clone, Serializable, Deserializable, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Expr {
    // --- 1. Primitives & Variables ---
    Val(JsonValue),
    Var(String),
    Now,

    // --- 2. Logique ---
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    IsA(String),

    // --- 3. Comparaisons ---
    Eq(Vec<Expr>),
    Neq(Vec<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gte(Box<Expr>, Box<Expr>),
    Lte(Box<Expr>, Box<Expr>),

    // --- 4. Mathématiques ---
    Add(Vec<Expr>),
    Sub(Vec<Expr>),
    Mul(Vec<Expr>),
    Div(Vec<Expr>),
    Abs(Box<Expr>),
    Round {
        value: Box<Expr>,
        precision: Box<Expr>,
    },

    // --- 5. Listes & Chaînes ---
    Len(Box<Expr>),
    Min(Box<Expr>),
    Max(Box<Expr>),
    Concat(Vec<Expr>),
    Contains {
        list: Box<Expr>,
        value: Box<Expr>,
    },

    // --- 6. String Ops ---
    Trim(Box<Expr>),
    Lower(Box<Expr>),
    Upper(Box<Expr>),
    RegexMatch {
        value: Box<Expr>,
        pattern: Box<Expr>,
    },
    Replace {
        value: Box<Expr>,
        pattern: Box<Expr>,
        replacement: Box<Expr>,
    },

    // --- 7. Fonctions Avancées (Extensions) ---
    Map {
        list: Box<Expr>,
        alias: String,
        expr: Box<Expr>,
    },
    Filter {
        list: Box<Expr>,
        alias: String,
        condition: Box<Expr>,
    },

    // --- 8. Dates ---
    DateDiff {
        start: Box<Expr>,
        end: Box<Expr>,
    },
    DateAdd {
        date: Box<Expr>,
        days: Box<Expr>,
    },

    // --- 9. Lookup ---
    Lookup {
        collection: String,
        id: Box<Expr>,
        field: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ast_serialization_primitive() -> RaiseResult<()> {
        let expr = Expr::Val(json_value!(42));

        let json = match json::serialize_to_string(&expr) {
            Ok(s) => s,
            Err(e) => raise_error!(
                "ERR_TEST_SERIALIZATION",
                error = e.to_string(),
                context = json_value!({ "target": "Expr::Val" })
            ),
        };

        assert_eq!(json, r#"{"val":42}"#);
        Ok(())
    }

    #[test]
    fn test_ast_deserialization_complex_rule() -> RaiseResult<()> {
        let json_str = r#"{
            "if": {
                "condition": { "gt": [{ "var": "sensors.temp" }, { "val": 100 }] },
                "then_branch": { "val": "ALERT" },
                "else_branch": { "val": "OK" }
            }
        }"#;

        let expr: Expr = match json::deserialize_from_str(json_str) {
            Ok(e) => e,
            Err(e) => raise_error!("ERR_TEST_DESERIALIZATION", error = e.to_string()),
        };

        match expr {
            Expr::If { .. } => assert!(true),
            _ => raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Structure incorrecte : Expr::If attendu"
            ),
        }
        Ok(())
    }

    #[test]
    fn test_rule_struct_compliance() -> RaiseResult<()> {
        let json_rule = r#"{
            "id": "RULE_001",
            "target": "oa.actors",
            "description": "Check",
            "expr": { "len": { "var": "name" } }
        }"#;

        let rule: Rule = match json::deserialize_from_str(json_rule) {
            Ok(r) => r,
            Err(e) => raise_error!("ERR_TEST_DESERIALIZATION", error = e.to_string()),
        };

        assert_eq!(rule.handle, "RULE_001");
        assert!(rule._id.is_none());
        Ok(())
    }

    #[test]
    fn test_ast_extensions() -> RaiseResult<()> {
        // Teste spécifiquement les variantes "Extensions" (Map, Filter, Regex)
        // pour garantir que le moteur supporte la logique complexe.

        // 1. Test Map
        let map_json = r#"{
            "map": {
                "list": { "var": "items" },
                "alias": "item",
                "expr": { "mul": [{ "var": "item.price" }, { "val": 1.2 }] }
            }
        }"#;

        let map_expr: Expr = match json::deserialize_from_str(map_json) {
            Ok(e) => e,
            Err(e) => raise_error!("ERR_TEST_DESERIALIZATION", error = e.to_string()),
        };

        if let Expr::Map { alias, .. } = map_expr {
            assert_eq!(alias, "item");
        } else {
            raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Structure incorrecte : Expr::Map attendu"
            );
        }

        // 2. Test RegexMatch
        let regex_json = r#"{
            "regex_match": {
                "value": { "var": "code" },
                "pattern": { "val": "^[A-Z]{3}-\\d{3}$" }
            }
        }"#;

        let regex_expr: Expr = match json::deserialize_from_str(regex_json) {
            Ok(e) => e,
            Err(e) => raise_error!("ERR_TEST_DESERIALIZATION", error = e.to_string()),
        };

        match regex_expr {
            Expr::RegexMatch { .. } => assert!(true),
            _ => raise_error!(
                "ERR_TEST_ASSERTION_FAILED",
                error = "Structure incorrecte : Expr::RegexMatch attendu"
            ),
        }
        Ok(())
    }
}
