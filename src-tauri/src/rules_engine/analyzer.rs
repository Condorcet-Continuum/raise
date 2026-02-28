// FICHIER : src-tauri/src/rules_engine/analyzer.rs

use crate::utils::{prelude::*, HashSet};

use crate::rules_engine::ast::Expr;

pub struct Analyzer;

impl Analyzer {
    pub fn get_dependencies(expr: &Expr) -> HashSet<String> {
        let mut deps = HashSet::new();
        let scope = Vec::new();
        Self::visit(expr, &mut deps, &scope);
        deps
    }

    pub fn validate_depth(expr: &Expr, max_depth: usize) -> RaiseResult<()> {
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

            // Opérateurs Unaires (Box<Expr>)
            Expr::Len(e)
            | Expr::Min(e)
            | Expr::Max(e)
            | Expr::Abs(e)
            | Expr::Not(e)
            | Expr::Trim(e)
            | Expr::Lower(e)
            | Expr::Upper(e) => {
                Self::visit(e, deps, scope);
            }

            // CORRECTION E0023 : Opérateurs N-aires (Vec<Expr>)
            // On traite Eq/Neq comme des listes, tout comme And/Or
            Expr::And(list)
            | Expr::Or(list)
            | Expr::Add(list)
            | Expr::Sub(list)
            | Expr::Mul(list)
            | Expr::Div(list)
            | Expr::Concat(list)
            | Expr::Eq(list)
            | Expr::Neq(list) => {
                for sub_expr in list {
                    Self::visit(sub_expr, deps, scope);
                }
            }

            // Opérateurs Binaires spécifiques (Struct variants ou Tuples)
            Expr::Contains { list, value }
            | Expr::RegexMatch {
                value: list, // Mapping astucieux : value -> list pour reusing la variable
                pattern: value,
            }
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
                Self::visit(list, deps, scope);
                Self::visit(value, deps, scope);
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::visit(condition, deps, scope);
                Self::visit(then_branch, deps, scope);
                Self::visit(else_branch, deps, scope);
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

            Expr::Round { value, precision } => {
                Self::visit(value, deps, scope);
                Self::visit(precision, deps, scope);
            }

            Expr::Lookup { id, .. } => Self::visit(id, deps, scope),
        }
    }

    fn check_depth(expr: &Expr, current: usize, max: usize) -> RaiseResult<()> {
        if current > max {
            raise_error!(
                "ERR_VALIDATION_MAX_DEPTH_EXCEEDED",
                error = format!(
                    "Limite de récursion atteinte : profondeur {} (max: {})",
                    current, max
                ),
                context = json!({
                    "current_depth": current,
                    "max_allowed": max,
                    "action": "enforce_recursion_limit",
                    "hint": "Une référence circulaire est probablement présente dans votre schéma ou vos données. Vérifiez les définitions récursives ($ref)."
                })
            );
        }

        match expr {
            Expr::Val(_) | Expr::Var(_) | Expr::Now => Ok(()),

            Expr::Map { list, expr, .. }
            | Expr::Filter {
                list,
                condition: expr,
                ..
            } => {
                Self::check_depth(list, current + 1, max)?;
                Self::check_depth(expr, current + 1, max)
            }

            Expr::Len(e)
            | Expr::Min(e)
            | Expr::Max(e)
            | Expr::Abs(e)
            | Expr::Not(e)
            | Expr::Trim(e)
            | Expr::Lower(e)
            | Expr::Upper(e) => Self::check_depth(e, current + 1, max),

            // CORRECTION E0023 : Traitement des listes pour Eq/Neq
            Expr::And(list)
            | Expr::Or(list)
            | Expr::Add(list)
            | Expr::Sub(list)
            | Expr::Mul(list)
            | Expr::Div(list)
            | Expr::Concat(list)
            | Expr::Eq(list)
            | Expr::Neq(list) => {
                for sub in list {
                    Self::check_depth(sub, current + 1, max)?;
                }
                Ok(())
            }

            Expr::Contains { list, value }
            | Expr::RegexMatch {
                value: list,
                pattern: value,
            }
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

            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::check_depth(condition, current + 1, max)?;
                Self::check_depth(then_branch, current + 1, max)?;
                Self::check_depth(else_branch, current + 1, max)
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

            Expr::Round { value, precision } => {
                Self::check_depth(value, current + 1, max)?;
                Self::check_depth(precision, current + 1, max)
            }

            Expr::Lookup { id, .. } => Self::check_depth(id, current + 1, max),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;
    use crate::utils::data::json;

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
        // Depth: Round (0) -> Abs (1) -> Sub (2) -> Val (3)
        assert!(Analyzer::validate_depth(&expr, 5).is_ok());
        assert!(Analyzer::validate_depth(&expr, 2).is_err());
    }

    #[test]
    fn test_dependencies_eq() {
        // Test que l'analyseur trouve les variables dans un Eq(Vec)
        // Expr: Eq([Var("a"), Var("b")])
        let expr = Expr::Eq(vec![Expr::Var("a".to_string()), Expr::Var("b".to_string())]);

        let deps = Analyzer::get_dependencies(&expr);
        assert!(deps.contains("a"));
        assert!(deps.contains("b"));
    }
}
