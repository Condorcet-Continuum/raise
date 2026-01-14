// FICHIER : src-tauri/src/rules_engine/ast.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Expr {
    // --- Primitives ---
    Val(serde_json::Value),
    Var(String),

    // --- 📦 Collections ---
    Len(Box<Expr>),
    Contains {
        list: Box<Expr>,
        value: Box<Expr>,
    },
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
    // NOUVEAU : Agrégations sur listes
    Min(Box<Expr>),
    Max(Box<Expr>),

    // --- Logique ---
    And(Vec<Expr>),
    Or(Vec<Expr>),
    Not(Box<Expr>),
    #[serde(rename = "if")]
    If {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
    Eq(Box<Expr>, Box<Expr>),
    Neq(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gte(Box<Expr>, Box<Expr>),
    Lte(Box<Expr>, Box<Expr>),

    // --- Mathématiques ---
    Add(Vec<Expr>),
    Sub(Vec<Expr>),
    Mul(Vec<Expr>),
    Div(Vec<Expr>),
    // NOUVEAU : Maths avancées
    Abs(Box<Expr>),
    Round {
        value: Box<Expr>,
        precision: Box<Expr>,
    },

    // --- 📅 Dates ---
    Now,
    DateDiff {
        start: Box<Expr>,
        end: Box<Expr>,
    },
    DateAdd {
        date: Box<Expr>,
        days: Box<Expr>,
    },

    // --- 🔤 Strings ---
    Concat(Vec<Expr>),
    Upper(Box<Expr>),
    // NOUVEAU : Manipulation de chaînes
    Lower(Box<Expr>),
    Trim(Box<Expr>),
    Replace {
        value: Box<Expr>,
        pattern: Box<Expr>,
        replacement: Box<Expr>,
    },
    RegexMatch {
        value: Box<Expr>,
        pattern: Box<Expr>,
    },

    // --- 🔍 Lookup ---
    Lookup {
        collection: String,
        id: Box<Expr>,
        field: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Rule {
    pub id: String,
    pub target: String,
    pub expr: Expr,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_ast_extensions() {
        // Test de sérialisation des nouveaux champs
        let expr = Expr::Round {
            value: Box::new(Expr::Val(json!(10.555))),
            precision: Box::new(Expr::Val(json!(2))),
        };
        let serialized = serde_json::to_string(&expr).unwrap();
        assert!(serialized.contains("round"));
        assert!(serialized.contains("precision"));
    }
}
