// FICHIER : src-tauri/src/json_db/query/sql.rs

use crate::json_db::transactions::TransactionRequest;

use crate::utils::data::Map;
use crate::utils::prelude::*;

use sqlparser::ast::{
    BinaryOperator, Expr, Insert, OrderByExpr, OrderByKind, Query as SqlQuery, SetExpr, Statement,
    TableFactor, Value as SqlValue,
};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;

use super::{
    ComparisonOperator, Condition, FilterOperator, Projection, Query, QueryFilter, SortField,
    SortOrder,
};

/// Résultat du parsing SQL : soit une lecture, soit une transaction d'écriture
pub enum SqlRequest {
    Read(Query),
    Write(Vec<TransactionRequest>),
}

pub fn parse_sql(sql: &str) -> Result<SqlRequest> {
    let dialect = GenericDialect {};

    let ast = Parser::parse_sql(&dialect, sql)
        .map_err(|e| AppError::Validation(format!("Erreur de syntaxe SQL : {}", e)))?;

    if ast.len() != 1 {
        return Err(AppError::Database(
            "Une seule requête SQL à la fois est supportée".to_string(),
        ));
    }

    match &ast[0] {
        Statement::Query(q) => {
            let query = translate_query(q)?;
            Ok(SqlRequest::Read(query))
        }
        // Utilisation du Variant Tuple Insert(Insert)
        Statement::Insert(insert) => {
            let tx = translate_insert(insert)?;
            Ok(SqlRequest::Write(tx))
        }
        _ => Err(AppError::Database(
            "Seuls SELECT et INSERT sont supportés pour le moment".to_string(),
        )),
    }
}

// --- TRADUCTION INSERT ---

fn translate_insert(insert: &Insert) -> Result<Vec<TransactionRequest>> {
    // CORRECTION DÉFINITIVE : Utilisation du champ `table`
    let collection = insert.table.to_string();

    let query_body = insert
        .source
        .as_ref()
        .ok_or_else(|| AppError::Validation("Clause VALUES manquante".to_string()))?
        .body
        .as_ref();

    let rows = match query_body {
        SetExpr::Values(v) => &v.rows,
        _ => {
            return Err(AppError::Database(
                "Seul INSERT INTO ... VALUES (...) est supporté".to_string(),
            ))
        }
    };

    let mut operations = Vec::new();

    for row in rows {
        if row.len() != insert.columns.len() {
            return Err(AppError::Database(format!(
                "Nombre de valeurs ({}) différent du nombre de colonnes ({})",
                row.len(),
                insert.columns.len()
            )));
        }

        let mut doc_map = Map::new();
        for (i, col_ident) in insert.columns.iter().enumerate() {
            let key = col_ident.value.clone();
            let val = expr_to_value(&row[i])?;
            doc_map.insert(key, val);
        }

        operations.push(TransactionRequest::Insert {
            collection: collection.clone(),
            id: None, // Laissez le manager générer l'UUID
            document: Value::Object(doc_map),
        });
    }

    Ok(operations)
}

fn translate_query(sql_query: &SqlQuery) -> Result<Query> {
    let query_string = sql_query.to_string().to_uppercase();

    // --- EXTRACTION DU LIMIT ---
    let mut limit = None;
    if let Some(idx) = query_string.find("LIMIT ") {
        let remainder = &query_string[idx + 6..];
        let digits: String = remainder
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        limit = digits.parse::<usize>().ok();
    }

    // --- EXTRACTION DE L'OFFSET ---
    let mut offset = None;
    if let Some(idx) = query_string.find("OFFSET ") {
        let remainder = &query_string[idx + 7..];
        let digits: String = remainder
            .chars()
            .skip_while(|c| c.is_whitespace())
            .take_while(|c| c.is_ascii_digit())
            .collect();
        offset = digits.parse::<usize>().ok();
    }
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
        _ => Err(AppError::Database(
            "Syntaxe de requête non supportée".to_string(),
        )),
    }
}

// --- TRADUCTION SELECT ---
fn translate_select(
    select: &sqlparser::ast::Select,
    limit: Option<usize>,
    offset: Option<usize>,
    sort: Option<Vec<SortField>>,
) -> Result<Query> {
    if select.from.len() != 1 {
        return Err(AppError::Database(
            "SELECT doit cibler exactement une collection".to_string(),
        ));
    }

    let collection = match &select.from[0].relation {
        TableFactor::Table { name, .. } => name.to_string(),
        _ => return Err(AppError::Database("Clause FROM invalide".to_string())),
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
                    _ => Condition::eq(field, value),
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
            Ok(QueryFilter {
                operator: FilterOperator::And,
                conditions: vec![Condition {
                    field,
                    operator: ComparisonOperator::Like,
                    value,
                }],
            })
        }
        _ => Err(AppError::Database(format!(
            "Expression SQL non supportée : {:?}",
            expr
        ))),
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
        _ => Err(AppError::Database(format!(
            "Identifiant attendu, obtenu : {:?}",
            expr
        ))),
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
                    Err(AppError::Database("Négation impossible".to_string()))
                }
            }
            _ => Err(AppError::Database(
                "Négation impossible sur non-nombre".to_string(),
            )),
        },
        _ => Err(AppError::Database(
            "Valeur littérale simple attendue".to_string(),
        )),
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
                    .map_err(|e| AppError::Database(format!("Float invalide: {}", e)))?;
                Ok(Value::from(f))
            }
        }
        SqlValue::SingleQuotedString(s) => Ok(Value::from(s.clone())),
        SqlValue::Boolean(b) => Ok(Value::from(*b)),
        _ => Ok(Value::Null),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json_db::transactions::TransactionRequest;

    #[test]
    fn test_parse_insert() {
        let sql = "INSERT INTO users (name, age) VALUES ('Alice', 30), ('Bob', 25)";
        let result = parse_sql(sql).unwrap();

        match result {
            SqlRequest::Write(ops) => {
                assert_eq!(ops.len(), 2);
                match &ops[0] {
                    TransactionRequest::Insert {
                        collection,
                        document,
                        ..
                    } => {
                        assert_eq!(collection, "users");
                        assert_eq!(document["name"], "Alice");
                        assert_eq!(document["age"], 30);
                    }
                    _ => panic!("Expected Insert op"),
                }
            }
            _ => panic!("Expected Write request"),
        }
    }

    #[test]
    fn test_parse_select_legacy() {
        let sql = "SELECT name FROM users WHERE age > 18";
        let result = parse_sql(sql).unwrap();
        match result {
            SqlRequest::Read(q) => assert_eq!(q.collection, "users"),
            _ => panic!("Expected Read request"),
        }
    }
}
