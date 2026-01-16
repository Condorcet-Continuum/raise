// FICHIER : src-tauri/src/rules_engine/ast.rs

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Représentation en mémoire d'une règle définie dans 'quality-rule.schema.json'.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub id: String,
    pub target: String,
    pub expr: Expr,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub severity: Option<String>,
}

/// Arbre Syntaxique Abstrait (AST) complet.
/// Union des fonctionnalités Legacy (Analyzer) et Modernes (Validator).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Expr {
    // --- 1. Primitives & Variables ---
    Val(Value),
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
    use serde_json::json;

    #[test]
    fn test_ast_serialization_primitive() {
        let expr = Expr::Val(json!(42));
        let json = serde_json::to_string(&expr).unwrap();
        assert_eq!(json, r#"{"val":42}"#);
    }

    #[test]
    fn test_ast_deserialization_complex_rule() {
        let json_str = r#"{
            "if": {
                "condition": { "gt": [{ "var": "sensors.temp" }, { "val": 100 }] },
                "then_branch": { "val": "ALERT" },
                "else_branch": { "val": "OK" }
            }
        }"#;
        let expr: Expr = serde_json::from_str(json_str).expect("Désérialisation échouée");
        match expr {
            Expr::If { .. } => assert!(true),
            _ => panic!("Structure incorrecte"),
        }
    }

    #[test]
    fn test_rule_struct_compliance() {
        let json_rule = r#"{
            "id": "RULE_001",
            "target": "oa.actors",
            "description": "Check",
            "expr": { "len": { "var": "name" } }
        }"#;
        let rule: Rule = serde_json::from_str(json_rule).unwrap();
        assert_eq!(rule.id, "RULE_001");
    }

    #[test]
    fn test_ast_extensions() {
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
        let map_expr: Expr = serde_json::from_str(map_json).unwrap();
        if let Expr::Map { alias, .. } = map_expr {
            assert_eq!(alias, "item");
        } else {
            panic!("Map non reconnu");
        }

        // 2. Test RegexMatch
        let regex_json = r#"{
            "regex_match": {
                "value": { "var": "code" },
                "pattern": { "val": "^[A-Z]{3}-\\d{3}$" }
            }
        }"#;
        let regex_expr: Expr = serde_json::from_str(regex_json).unwrap();
        match regex_expr {
            Expr::RegexMatch { .. } => assert!(true),
            _ => panic!("RegexMatch non reconnu"),
        }
    }
}
