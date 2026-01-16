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
    let limit = None;
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
        _ => bail!("Syntaxe de requête non supportée"),
    }
}

fn translate_select(
    select: &sqlparser::ast::Select,
    limit: Option<usize>,
    offset: Option<usize>,
    sort: Option<Vec<SortField>>,
) -> Result<Query> {
    if select.from.len() != 1 {
        bail!("SELECT doit cibler exactement une collection");
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

fn translate_expr(expr: &Expr) -> Result<QueryFilter> {
    match expr {
        Expr::Nested(inner) => translate_expr(inner),
        Expr::BinaryOp { left, op, right } => match op {
            BinaryOperator::And => {
                let l = translate_expr(left)?;
                let r = translate_expr(right)?;
                let mut conds = l.conditions;
                conds.extend(r.conditions);
                Ok(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: conds,
                })
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
            _ => {
                let field = expr_to_field_name(left)?;
                let value = expr_to_value(right)?;
                let condition = match op {
                    BinaryOperator::Eq => Condition::eq(field, value),
                    BinaryOperator::NotEq => Condition::ne(field, value),
                    BinaryOperator::Gt => Condition::gt(field, value),
                    BinaryOperator::GtEq => Condition::gte(field, value),
                    BinaryOperator::Lt => Condition::lt(field, value),
                    BinaryOperator::LtEq => Condition::lte(field, value),
                    _ => Condition::eq(field, value), // Fallback
                };
                Ok(QueryFilter {
                    operator: FilterOperator::And,
                    conditions: vec![condition],
                })
            }
        },
        Expr::Like { expr, pattern, .. } => {
            let field = expr_to_field_name(expr)?;
            let value = expr_to_value(pattern)?;
            // Mappe SQL LIKE vers Condition::like (ou contains selon implémentation)
            Ok(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition {
                    field,
                    operator: ComparisonOperator::Like,
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
        _ => bail!("Identifiant attendu, obtenu : {:?}", expr),
    }
}

fn expr_to_value(expr: &Expr) -> Result<Value> {
    match expr {
        Expr::Value(value_with_span) => sql_value_to_json(&value_with_span.value),
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
                    bail!("Négation impossible")
                }
            }
            _ => bail!("Négation impossible sur non-nombre"),
        },
        _ => bail!("Valeur littérale simple attendue"),
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

// ============================================================================
// TESTS UNITAIRES
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_select() {
        let q = parse_sql("SELECT name FROM users WHERE age > 18").unwrap();
        assert_eq!(q.collection, "users");

        // Vérification du filtre
        let filter = q.filter.unwrap();
        assert_eq!(filter.conditions.len(), 1);
        assert_eq!(filter.conditions[0].field, "age");
        assert!(matches!(
            filter.conditions[0].operator,
            ComparisonOperator::Gt
        ));

        // Vérification de la projection
        match q.projection.unwrap() {
            Projection::Include(fields) => assert_eq!(fields[0], "name"),
            _ => panic!("Projection failed"),
        }
    }

    #[test]
    fn test_parse_and_logic() {
        let q = parse_sql("SELECT * FROM t WHERE a = 1 AND b = 2").unwrap();
        let filter = q.filter.unwrap();
        assert_eq!(filter.conditions.len(), 2);
    }
}
