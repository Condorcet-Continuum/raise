// FICHIER : src-tauri/src/rules_engine/analyzer.rs

use crate::rules_engine::ast::Expr;
use crate::utils::prelude::*;

pub struct Analyzer;

impl Analyzer {
    /// Extrait les dépendances en garantissant qu'aucun débordement de pile (Stack Overflow)
    /// ne peut se produire grâce au paramètre `max_depth`.
    pub fn get_dependencies(expr: &Expr, max_depth: usize) -> RaiseResult<UniqueSet<String>> {
        let mut deps = UniqueSet::new();
        let scope = Vec::new();
        Self::visit(expr, &mut deps, &scope, 0, max_depth)?;
        Ok(deps)
    }

    /// Rétrocompatibilité : Valide uniquement la profondeur sans extraire les dépendances.
    pub fn validate_depth(expr: &Expr, max_depth: usize) -> RaiseResult<()> {
        let mut dummy_deps = UniqueSet::new();
        Self::visit(expr, &mut dummy_deps, &Vec::new(), 0, max_depth)
    }

    /// Parcours fusionné : Extrait les dépendances ET vérifie la profondeur en O(N)
    fn visit(
        expr: &Expr,
        deps: &mut UniqueSet<String>,
        scope: &Vec<String>,
        current_depth: usize,
        max_depth: usize,
    ) -> RaiseResult<()> {
        // 🛡️ GARDE DE SÉCURITÉ ANTI-STACK OVERFLOW
        if current_depth > max_depth {
            raise_error!(
                "ERR_VALIDATION_MAX_DEPTH_EXCEEDED",
                error = format!(
                    "Limite de récursion atteinte : profondeur {} (max: {})",
                    current_depth, max_depth
                ),
                context = json_value!({
                    "current_depth": current_depth,
                    "max_allowed": max_depth,
                    "action": "enforce_recursion_limit",
                    "hint": "Une référence circulaire est probablement présente dans votre schéma ou vos données."
                })
            );
        }

        match expr {
            Expr::Val(_) | Expr::Now | Expr::IsA(_) => Ok(()),

            Expr::Var(name) => {
                let is_local = scope.iter().any(|local_var| {
                    name == local_var || name.starts_with(&format!("{}.", local_var))
                });
                if !is_local {
                    deps.insert(name.clone());
                }
                Ok(())
            }

            // Collections & Scopes
            Expr::Map {
                list,
                alias,
                expr: sub_expr,
            }
            | Expr::Filter {
                list,
                alias,
                condition: sub_expr,
            } => {
                Self::visit(list, deps, scope, current_depth + 1, max_depth)?;
                let mut new_scope = scope.clone();
                new_scope.push(alias.clone());
                Self::visit(sub_expr, deps, &new_scope, current_depth + 1, max_depth)
            }

            // Opérateurs Unaires
            Expr::Len(e)
            | Expr::Min(e)
            | Expr::Max(e)
            | Expr::Abs(e)
            | Expr::Not(e)
            | Expr::Trim(e)
            | Expr::Lower(e)
            | Expr::Upper(e) => Self::visit(e, deps, scope, current_depth + 1, max_depth),

            // Opérateurs N-aires
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
                    Self::visit(sub_expr, deps, scope, current_depth + 1, max_depth)?;
                }
                Ok(())
            }

            // Opérateurs Binaires spécifiques
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
                Self::visit(list, deps, scope, current_depth + 1, max_depth)?;
                Self::visit(value, deps, scope, current_depth + 1, max_depth)
            }

            Expr::Round { value, precision } => {
                Self::visit(value, deps, scope, current_depth + 1, max_depth)?;
                Self::visit(precision, deps, scope, current_depth + 1, max_depth)
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
            } => {
                Self::visit(condition, deps, scope, current_depth + 1, max_depth)?;
                Self::visit(then_branch, deps, scope, current_depth + 1, max_depth)?;
                Self::visit(else_branch, deps, scope, current_depth + 1, max_depth)
            }

            Expr::Replace {
                value,
                pattern,
                replacement,
            } => {
                Self::visit(value, deps, scope, current_depth + 1, max_depth)?;
                Self::visit(pattern, deps, scope, current_depth + 1, max_depth)?;
                Self::visit(replacement, deps, scope, current_depth + 1, max_depth)
            }

            Expr::Lookup { id, .. } => Self::visit(id, deps, scope, current_depth + 1, max_depth),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules_engine::ast::Expr;
    use crate::utils::core::error::AppError;

    #[test]
    fn test_depth_validation_success_and_failure() -> RaiseResult<()> {
        let expr = Expr::Round {
            value: Box::new(Expr::Abs(Box::new(Expr::Sub(vec![
                Expr::Val(json_value!(10)),
                Expr::Val(json_value!(20)),
            ])))),
            precision: Box::new(Expr::Val(json_value!(2))),
        };

        // La profondeur requise est de 3 (Round -> Abs -> Sub -> Val)
        assert!(
            Analyzer::validate_depth(&expr, 5).is_ok(),
            "Devrait passer avec une limite de 5"
        );

        let err_res = Analyzer::validate_depth(&expr, 2);
        assert!(err_res.is_err(), "Devrait échouer avec une limite de 2");

        // Vérification stricte du code d'erreur
        if let Err(AppError::Structured(err)) = err_res {
            assert_eq!(err.code, "ERR_VALIDATION_MAX_DEPTH_EXCEEDED");
        } else {
            raise_error!(
                "ERR_TEST_FAILED",
                error = "Le type d'erreur retourné est incorrect."
            );
        }

        Ok(())
    }

    #[test]
    fn test_dependencies_extraction() -> RaiseResult<()> {
        let expr = Expr::Eq(vec![Expr::Var("a".to_string()), Expr::Var("b".to_string())]);

        let deps = Analyzer::get_dependencies(&expr, 10)?;
        assert!(deps.contains("a"));
        assert!(deps.contains("b"));
        assert_eq!(deps.len(), 2);

        Ok(())
    }

    #[test]
    fn test_dependencies_with_local_scope_shadowing() -> RaiseResult<()> {
        // Règle : MAP sur "items" as "item", on fait item.price * tax_rate
        // Dépendances attendues : "items" et "tax_rate". "item" est local.
        let expr = Expr::Map {
            list: Box::new(Expr::Var("items".to_string())),
            alias: "item".to_string(),
            expr: Box::new(Expr::Mul(vec![
                Expr::Var("item.price".to_string()),
                Expr::Var("tax_rate".to_string()),
            ])),
        };

        let deps = Analyzer::get_dependencies(&expr, 10)?;
        assert!(deps.contains("items"));
        assert!(deps.contains("tax_rate"));
        assert!(
            !deps.contains("item.price"),
            "La variable locale ne doit pas fuiter comme dépendance externe"
        );
        assert_eq!(deps.len(), 2);

        Ok(())
    }

    #[test]
    fn test_stack_overflow_prevention() -> RaiseResult<()> {
        // On crée manuellement un AST très profond (ex: Not(Not(Not(...))))
        let mut deep_expr = Expr::Val(json_value!(true));
        for _ in 0..50 {
            deep_expr = Expr::Not(Box::new(deep_expr));
        }

        // On impose une limite très stricte de 10
        let res = Analyzer::get_dependencies(&deep_expr, 10);

        match res {
            Err(AppError::Structured(err)) => {
                assert_eq!(err.code, "ERR_VALIDATION_MAX_DEPTH_EXCEEDED");
                Ok(())
            }
            _ => raise_error!(
                "ERR_TEST_FAILED",
                error = "L'analyseur n'a pas bloqué le débordement de profondeur."
            ),
        }
    }
}
