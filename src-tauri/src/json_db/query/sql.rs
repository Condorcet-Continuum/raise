// FICHIER : src-tauri/src/json_db/query/sql.rs

use anyhow::{bail, Result};
use serde_json::Value;
use sqlparser::ast::{
    BinaryOperator, Expr, OrderByExpr, OrderByKind, Query as SqlQuery, SetExpr, Statement,
    TableFactor, Value as SqlValue,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use super::{
    ComparisonOperator, Condition, FilterOperator, Projection, Query, QueryFilter, SortField,
    SortOrder,
};

pub fn parse_sql(sql: &str) -> Result<Query> {
    let dialect = GenericDialect {};
    let ast = Parser::parse_sql(&dialect, sql)?;

    if ast.len() != 1 {
        bail!("Une seule requête SQL à la fois est supportée");
    }

    match &ast[0] {
        Statement::Query(q) => translate_query(q),
        _ => bail!("Seules les requêtes SELECT sont supportées pour le moment"),
    }
}

fn translate_query(sql_query: &SqlQuery) -> Result<Query> {
    let limit = None; // Désactivé temporairement (compatibilité versions sqlparser)
    let offset = None;

    let sort = if let Some(order_by_struct) = &sql_query.order_by {
        match &order_by_struct.kind {
            OrderByKind::Expressions(exprs) => {
                let mut fields = Vec::new();
                for order_expr in exprs {
                    fields.push(translate_order_by(order_expr)?);
                }
                if fields.is_empty() {
                    None
                } else {
                    Some(fields)
                }
            }
            _ => None,
        }
    } else {
        None
    };

    match &*sql_query.body {
        SetExpr::Select(select) => translate_select(select, limit, offset, sort),
        _ => bail!("Syntaxe de requête non supportée (pas de UNION, VALUES, etc.)"),
    }
}

fn translate_select(
    select: &sqlparser::ast::Select,
    limit: Option<usize>,
    offset: Option<usize>,
    sort: Option<Vec<SortField>>,
) -> Result<Query> {
    if select.from.len() != 1 {
        bail!("SELECT doit cibler exactement une collection (pas de JOIN supporté)");
    }

    let collection = match &select.from[0].relation {
        TableFactor::Table { name, .. } => name.to_string(),
        _ => bail!("Clause FROM invalide"),
    };

    let projection = if select.projection.is_empty() {
        None
    } else {
        let mut fields = Vec::new();
        let mut is_wildcard = false;

        for item in &select.projection {
            match item {
                sqlparser::ast::SelectItem::UnnamedExpr(Expr::Identifier(ident)) => {
                    fields.push(ident.value.clone());
                }
                sqlparser::ast::SelectItem::UnnamedExpr(Expr::CompoundIdentifier(idents)) => {
                    fields.push(
                        idents
                            .iter()
                            .map(|i| i.value.clone())
                            .collect::<Vec<_>>()
                            .join("."),
                    );
                }
                sqlparser::ast::SelectItem::Wildcard(_) => {
                    is_wildcard = true;
                    break;
                }
                _ => {}
            }
        }

        if is_wildcard || fields.is_empty() {
            None
        } else {
            Some(Projection::Include(fields))
        }
    };

    let filter = if let Some(selection) = &select.selection {
        Some(translate_expr(selection)?)
    } else {
        None
    };

    Ok(Query {
        collection,
        filter,
        sort,
        limit,
        offset,
        projection,
    })
}

fn translate_order_by(expr: &OrderByExpr) -> Result<SortField> {
    let field = expr_to_field_name(&expr.expr)?;
    let order = match expr.options.asc {
        Some(false) => SortOrder::Desc,
        _ => SortOrder::Asc,
    };
    Ok(SortField { field, order })
}

/// Traduit une expression SQL (WHERE clause) en QueryFilter
fn translate_expr(expr: &Expr) -> Result<QueryFilter> {
    match expr {
        // 1. Gestion des parenthèses
        Expr::Nested(inner) => translate_expr(inner),

        // 2. Gestion des Opérateurs Binaires
        Expr::BinaryOp { left, op, right } => match op {
            // --- LOGIQUE (AND / OR) ---
            BinaryOperator::And => {
                let l = translate_expr(left)?;
                let r = translate_expr(right)?;
                // Fusion des conditions si l'opérateur est identique
                if matches!(l.operator, FilterOperator::And)
                    && matches!(r.operator, FilterOperator::And)
                {
                    let mut conds = l.conditions;
                    conds.extend(r.conditions);
                    Ok(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: conds,
                    })
                } else {
                    // TODO: Gérer l'imbrication complexe (AND contenant des OR).
                    // Pour l'instant, on aplatit au mieux.
                    let mut conds = l.conditions;
                    conds.extend(r.conditions);
                    Ok(QueryFilter {
                        operator: FilterOperator::And,
                        conditions: conds,
                    })
                }
            }
            BinaryOperator::Or => {
                let l = translate_expr(left)?;
                let r = translate_expr(right)?;
                let mut conds = l.conditions;
                conds.extend(r.conditions);
                Ok(QueryFilter {
                    operator: FilterOperator::Or,
                    conditions: conds,
                })
            }

            // --- COMPARAISON (>, <, =, !=) ---
            _ => {
                // C'est ici que ça plantait : on ne cherche un nom de champ QUE si c'est une comparaison
                let field = expr_to_field_name(left)?;
                let value = expr_to_value(right)?;

                let operator = match op {
                    BinaryOperator::Eq => ComparisonOperator::Eq,
                    BinaryOperator::NotEq => ComparisonOperator::Ne,
                    BinaryOperator::Gt => ComparisonOperator::Gt,
                    BinaryOperator::GtEq => ComparisonOperator::Gte,
                    BinaryOperator::Lt => ComparisonOperator::Lt,
                    BinaryOperator::LtEq => ComparisonOperator::Lte,
                    _ => ComparisonOperator::Eq,
                };

                Ok(QueryFilter {
                    operator: FilterOperator::And, // Un filtre atomique est un AND de 1 condition
                    conditions: vec![Condition {
                        field,
                        operator,
                        value,
                    }],
                })
            }
        },

        // 3. Gestion LIKE
        Expr::Like { expr, pattern, .. } => {
            let field = expr_to_field_name(expr)?;
            let value = expr_to_value(pattern)?;
            Ok(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition {
                    field,
                    operator: ComparisonOperator::Contains,
                    value,
                }],
            })
        }

        _ => bail!("Expression SQL non supportée : {:?}", expr),
    }
}

fn expr_to_field_name(expr: &Expr) -> Result<String> {
    match expr {
        Expr::Identifier(ident) => Ok(ident.value.clone()),
        Expr::CompoundIdentifier(idents) => Ok(idents
            .iter()
            .map(|i| i.value.clone())
            .collect::<Vec<_>>()
            .join(".")),
        _ => bail!(
            "Champ attendu (identifiant simple ou composé), obtenu : {:?}",
            expr
        ),
    }
}

fn expr_to_value(expr: &Expr) -> Result<Value> {
    match expr {
        // ValueWithSpan wrapper dans les versions récentes de sqlparser
        Expr::Value(value_with_span) => sql_value_to_json(&value_with_span.value),

        // Support des nombres négatifs (-10)
        Expr::UnaryOp {
            op: sqlparser::ast::UnaryOperator::Minus,
            expr: inner,
        } => match expr_to_value(inner)? {
            Value::Number(n) => {
                if let Some(f) = n.as_f64() {
                    Ok(Value::from(-f))
                } else if let Some(i) = n.as_i64() {
                    Ok(Value::from(-i))
                } else {
                    bail!("Négation impossible sur ce type")
                }
            }
            _ => bail!("Négation impossible sur non-nombre"),
        },
        _ => bail!("Valeur littérale simple attendue (pas d'expression complexe)"),
    }
}

#[allow(dead_code)]
fn expr_to_usize(expr: &Expr) -> Result<usize> {
    match expr {
        Expr::Value(value_with_span) => match &value_with_span.value {
            SqlValue::Number(n, _) => n
                .parse::<usize>()
                .map_err(|e| anyhow::anyhow!("Erreur parsing: {}", e)),
            _ => bail!("Nombre attendu"),
        },
        _ => bail!("Expression simple attendue"),
    }
}

fn sql_value_to_json(val: &SqlValue) -> Result<Value> {
    match val {
        SqlValue::Number(n, _) => {
            if let Ok(i) = n.parse::<i64>() {
                Ok(Value::from(i))
            } else {
                let f: f64 = n
                    .parse()
                    .map_err(|e| anyhow::anyhow!("Float invalide: {}", e))?;
                Ok(Value::from(f))
            }
        }
        SqlValue::SingleQuotedString(s) => Ok(Value::from(s.clone())),
        SqlValue::Boolean(b) => Ok(Value::from(*b)),
        _ => Ok(Value::Null),
    }
}
