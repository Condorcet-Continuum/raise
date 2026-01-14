// FICHIER : src-tauri/src/rules_engine/analyzer.rs

use crate::rules_engine::ast::Expr;
use std::collections::HashSet;

pub struct Analyzer;

impl Analyzer {
    pub fn get_dependencies(expr: &Expr) -> HashSet<String> {
        let mut deps = HashSet::new();
        let scope = Vec::new();
        Self::visit(expr, &mut deps, &scope);
        deps
    }

    pub fn validate_depth(expr: &Expr, max_depth: usize) -> Result<(), String> {
        Self::check_depth(expr, 0, max_depth)
    }

    fn visit(expr: &Expr, deps: &mut HashSet<String>, scope: &Vec<String>) {
        match expr {
            Expr::Val(_) | Expr::Now => {}

            Expr::Var(name) => {
                let is_local = scope.iter().any(|local_var| {
                    name == local_var || name.starts_with(&format!("{}.", local_var))
                });
                if !is_local {
                    deps.insert(name.clone());
                }
            }

            // Collections & Scopes
            Expr::Map { list, alias, expr }
            | Expr::Filter {
                list,
                alias,
                condition: expr,
            } => {
                Self::visit(list, deps, scope);
                let mut new_scope = scope.clone();
                new_scope.push(alias.clone());
                Self::visit(expr, deps, &new_scope);
            }
            Expr::Len(e) | Expr::Min(e) | Expr::Max(e) => Self::visit(e, deps, scope),
            Expr::Contains { list, value } => {
                Self::visit(list, deps, scope);
                Self::visit(value, deps, scope);
            }

            // Listes génériques
            Expr::And(l)
            | Expr::Or(l)
            | Expr::Add(l)
            | Expr::Sub(l)
            | Expr::Mul(l)
            | Expr::Div(l)
            | Expr::Concat(l) => {
                for item in l {
                    Self::visit(item, deps, scope);
                }
            }

            // Unaires
            Expr::Not(e) | Expr::Upper(e) | Expr::Lower(e) | Expr::Trim(e) | Expr::Abs(e) => {
                Self::visit(e, deps, scope)
            }

            // Structures complexes
            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::visit(condition, deps, scope);
                Self::visit(then_branch, deps, scope);
                Self::visit(else_branch, deps, scope);
            }
            Expr::Round { value, precision } => {
                Self::visit(value, deps, scope);
                Self::visit(precision, deps, scope);
            }
            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                Self::visit(value, deps, scope);
                Self::visit(pattern, deps, scope);
                Self::visit(replacement, deps, scope);
            }

            // Binaires standards
            Expr::Eq(a, b)
            | Expr::Neq(a, b)
            | Expr::Gt(a, b)
            | Expr::Lt(a, b)
            | Expr::Gte(a, b)
            | Expr::Lte(a, b)
            | Expr::DateDiff { start: a, end: b }
            | Expr::DateAdd { date: a, days: b }
            | Expr::RegexMatch {
                value: a,
                pattern: b,
            } => {
                Self::visit(a, deps, scope);
                Self::visit(b, deps, scope);
            }

            Expr::Lookup { id, .. } => {
                Self::visit(id, deps, scope);
            }
        }
    }

    fn check_depth(expr: &Expr, current: usize, max: usize) -> Result<(), String> {
        if current > max {
            return Err(format!("Expression too deep (max {})", max));
        }
        match expr {
            Expr::Val(_) | Expr::Now | Expr::Var(_) => Ok(()),

            Expr::And(l)
            | Expr::Or(l)
            | Expr::Add(l)
            | Expr::Sub(l)
            | Expr::Mul(l)
            | Expr::Div(l)
            | Expr::Concat(l) => {
                for item in l {
                    Self::check_depth(item, current + 1, max)?;
                }
                Ok(())
            }

            // Mises à jour des variantes unaires
            Expr::Not(e)
            | Expr::Upper(e)
            | Expr::Lower(e)
            | Expr::Trim(e)
            | Expr::Abs(e)
            | Expr::Len(e)
            | Expr::Min(e)
            | Expr::Max(e) => Self::check_depth(e, current + 1, max),

            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::check_depth(condition, current + 1, max)?;
                Self::check_depth(then_branch, current + 1, max)?;
                Self::check_depth(else_branch, current + 1, max)
            }

            Expr::Map { list, expr, .. }
            | Expr::Filter {
                list,
                condition: expr,
                ..
            } => {
                Self::check_depth(list, current + 1, max)?;
                Self::check_depth(expr, current + 1, max)
            }

            Expr::Round { value, precision } => {
                Self::check_depth(value, current + 1, max)?;
                Self::check_depth(precision, current + 1, max)
            }

            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                Self::check_depth(value, current + 1, max)?;
                Self::check_depth(pattern, current + 1, max)?;
                Self::check_depth(replacement, current + 1, max)
            }

            Expr::Contains { list, value }
            | Expr::RegexMatch {
                value: list,
                pattern: value,
            }
            | Expr::Eq(list, value)
            | Expr::Neq(list, value)
            | Expr::Gt(list, value)
            | Expr::Lt(list, value)
            | Expr::Gte(list, value)
            | Expr::Lte(list, value)
            | Expr::DateDiff {
                start: list,
                end: value,
            }
            | Expr::DateAdd {
                date: list,
                days: value,
            } => {
                Self::check_depth(list, current + 1, max)?;
                Self::check_depth(value, current + 1, max)
            }

            Expr::Lookup { id, .. } => Self::check_depth(id, current + 1, max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;
    use serde_json::json;

    #[test]
    fn test_new_functions_depth() {
        // Test de profondeur sur une fonction imbriquée complexe
        let expr = Expr::Round {
            value: Box::new(Expr::Abs(Box::new(Expr::Sub(vec![
                Expr::Val(json!(10)),
                Expr::Val(json!(20)),
            ])))),
            precision: Box::new(Expr::Val(json!(2))),
        };
        // Depth: Round(1) -> Abs(2) -> Sub(3)
        assert!(Analyzer::validate_depth(&expr, 5).is_ok());
        assert!(Analyzer::validate_depth(&expr, 2).is_err());
    }
}
