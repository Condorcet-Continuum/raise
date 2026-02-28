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

pub fn parse_sql(sql: &str) -> RaiseResult<SqlRequest> {
    let dialect = GenericDialect {};

    let ast = match Parser::parse_sql(&dialect, sql) {
        Ok(tree) => tree,
        // On ajoute les accolades ici pour encadrer la macro divergente
        Err(e) => {
            raise_error!(
                "ERR_DB_SQL_SYNTAX",
                error = e,
                context = json!({
                    "sql_query": sql,
                    "dialect": format!("{:?}", dialect),
                    "action": "generate_sql_ast"
                })
            )
        } // Pas de virgule ou point-virgule nécessaire ici si c'est le dernier bras
    };

    if ast.len() != 1 {
        raise_error!(
            "ERR_DB_SQL_SINGLE_STATEMENT_ONLY",
            error = "Multi-statement execution non supportée : une seule requête SQL est autorisée par appel.",
            context = json!({
                "statements_count": ast.len(),
                "action": "validate_sql_batch_size",
                "hint": "Veuillez séparer vos requêtes et les exécuter séquentiellement."
            })
        );
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
        // Cas non supportés : Levée d'une erreur structurée
        unsupported => {
            raise_error!(
                "ERR_DB_SQL_STATEMENT_UNSUPPORTED",
                error = "Type de requête SQL non supporté par le moteur actuel.",
                context = json!({
                    "attempted_statement": format!("{:?}", unsupported),
                    "supported_statements": ["SELECT", "INSERT"],
                    "action": "translate_sql_to_request",
                    "hint": "Le moteur JSON-DB est actuellement limité aux opérations de lecture (SELECT) et d'insertion (INSERT)."
                })
            );
        }
    }
}

// --- TRADUCTION INSERT ---

fn translate_insert(insert: &Insert) -> RaiseResult<Vec<TransactionRequest>> {
    let collection = insert.table.to_string();

    let Some(source) = insert.source.as_ref() else {
        raise_error!(
            "ERR_DB_SQL_INSERT_SOURCE_MISSING",
            error = "Instruction INSERT invalide : clause VALUES ou source de données manquante.",
            context = json!({
                "table_name": collection,
                "action": "parse_insert_statement",
                "hint": "Assurez-vous que votre requête contient une clause 'VALUES' ou 'SELECT' pour alimenter l'insertion."
            })
        );
    };

    let query_body = source.body.as_ref();
    let SetExpr::Values(v) = query_body else {
        raise_error!(
            "ERR_DB_SQL_INSERT_TYPE_UNSUPPORTED",
            error = "Format d'insertion non supporté : seul 'INSERT INTO ... VALUES' est autorisé.",
            context = json!({
                "attempted_body_type": format!("{:?}", query_body),
                "action": "parse_insert_values",
                "hint": "Les insertions basées sur des sous-requêtes (SELECT) ne sont pas encore supportées."
            })
        );
    };
    let rows = &v.rows;
    let mut operations = Vec::new();

    for row in rows {
        if row.len() != insert.columns.len() {
            raise_error!(
                "ERR_DB_SQL_INSERT_COLUMNS_MISMATCH",
                error = format!(
                    "Déséquilibre lors de l'insertion : {} valeurs fournies pour {} colonnes.",
                    row.len(),
                    insert.columns.len()
                ),
                context = json!({
                    "expected_columns": insert.columns.iter().map(|c| c.value.to_string()).collect::<Vec<_>>(),
                    "values_count": row.len(),
                    "columns_count": insert.columns.len(),
                    "action": "validate_insert_row_alignment"
                })
            );
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

fn translate_query(sql_query: &SqlQuery) -> RaiseResult<Query> {
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
        unsupported_expr => {
            raise_error!(
                "ERR_DB_SQL_SELECT_EXPR_UNSUPPORTED",
                error = "Structure de requête SELECT non supportée (seul le mode SELECT simple est autorisé).",
                context = json!({
                    "attempted_expression": format!("{:?}", unsupported_expr),
                    "action": "translate_sql_body",
                    "hint": "Les opérations de type UNION, EXCEPT ou INTERSECT ne sont pas encore supportées par le moteur JSON-DB."
                })
            );
        }
    }
}

// --- TRADUCTION SELECT ---
fn translate_select(
    select: &sqlparser::ast::Select,
    limit: Option<usize>,
    offset: Option<usize>,
    sort: Option<Vec<SortField>>,
) -> RaiseResult<Query> {
    if select.from.len() != 1 {
        raise_error!(
            "ERR_DB_SQL_MULTIPLE_SOURCES_UNSUPPORTED",
            error = "Le moteur JSON-DB ne supporte qu'une seule collection par requête (JOIN non supporté).",
            context = json!({
                "sources_found": select.from.iter().map(|f| f.relation.to_string()).collect::<Vec<_>>(),
                "sources_count": select.from.len(),
                "action": "validate_select_from_clause",
                "hint": "Veuillez simplifier votre requête pour ne cibler qu'une seule collection '@collection'."
            })
        );
    }

    let TableFactor::Table { name, .. } = &select.from[0].relation else {
        raise_error!(
            "ERR_DB_SQL_FROM_RELATION_UNSUPPORTED",
            error = "La clause FROM est invalide ou utilise une structure non supportée (sous-requêtes, jointures).",
            context = json!({
                "attempted_relation": format!("{:?}", select.from[0].relation),
                "action": "resolve_collection_name",
                "hint": "Utilisez un nom de collection simple : SELECT * FROM ma_collection"
            })
        );
    };

    let collection = name.to_string();

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

fn translate_order_by(expr: &OrderByExpr) -> RaiseResult<SortField> {
    let field = expr_to_field_name(&expr.expr)?;
    let order = match expr.options.asc {
        Some(false) => SortOrder::Desc,
        _ => SortOrder::Asc,
    };
    Ok(SortField { field, order })
}

fn translate_expr(expr: &Expr) -> RaiseResult<QueryFilter> {
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
        _ => {
            raise_error!(
                "ERR_DB_SQL_EXPRESSION_UNSUPPORTED",
                error = "Expression SQL non supportée par le moteur de filtrage actuel.",
                context = json!({
                    "attempted_expression": format!("{:?}", expr),
                    "action": "translate_sql_expression",
                    "hint": "Le moteur supporte actuellement les comparaisons simples (=, !=, <, >, <=, >=) et les opérateurs logiques (AND, OR)."
                })
            );
        }
    }
}

fn expr_to_field_name(expr: &Expr) -> RaiseResult<String> {
    match expr {
        Expr::Identifier(ident) => Ok(ident.value.clone()),
        Expr::CompoundIdentifier(idents) => Ok(idents
            .iter()
            .map(|i| i.value.clone())
            .collect::<Vec<_>>()
            .join(".")),
        _ => {
            raise_error!(
                "ERR_DB_SQL_IDENTIFIER_EXPECTED",
                error = "Identifiant de champ invalide ou non supporté.",
                context = json!({
                    "received_expression_type": format!("{:?}", expr),
                    "action": "resolve_column_identifier",
                    "hint": "Un nom de champ simple est attendu ici. Les fonctions, calculs ou sous-requêtes ne sont pas autorisés à cet emplacement."
                })
            );
        }
    }
}

fn expr_to_value(expr: &Expr) -> RaiseResult<Value> {
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
                    raise_error!(
                        "ERR_DB_SQL_NEGATION_UNSUPPORTED",
                        error = "L'opérateur de négation 'NOT' ne peut pas être appliqué à cette expression.",
                        context = json!({
                            "expression_context": format!("{:?}", expr),
                            "action": "translate_not_operator",
                            "hint": "La négation est actuellement limitée aux comparaisons directes (ex: NOT field = value)."
                        })
                    );
                }
            }
            _ => {
                raise_error!(
                    "ERR_DB_SQL_UNARY_MINUS_TYPE_MISMATCH",
                    error = "Opération arithmétique invalide : la négation (-) requiert une valeur numérique.",
                    context = json!({
                        "received_expression": format!("{:?}", expr),
                        "action": "apply_unary_minus",
                        "hint": "Vérifiez que le champ ciblé contient des nombres. Les chaînes de caractères et les booléens ne peuvent pas être inversés avec '-'."
                    })
                );
            }
        },
        _ => {
            raise_error!(
                "ERR_DB_SQL_LITERAL_EXPECTED",
                error = "Expression invalide : une valeur littérale simple (string, nombre, bool) est attendue.",
                context = json!({
                    "attempted_expression": format!("{:?}", expr),
                    "action": "translate_sql_value",
                    "hint": "Le moteur JSON-DB ne supporte pas encore les expressions calculées dans cette clause. Utilisez une valeur directe."
                })
            );
        }
    }
}

fn sql_value_to_json(val: &SqlValue) -> RaiseResult<Value> {
    match val {
        SqlValue::Number(n, _) => {
            if let Ok(i) = n.parse::<i64>() {
                Ok(Value::from(i))
            } else {
                let f: f64 = match n.parse() {
                    Ok(num) => num,
                    Err(e) => {
                        raise_error!(
                            "ERR_DB_NUMERIC_PARSE_FAIL",
                            error = e,
                            context = json!({
                                "input_string": n,
                                "target_type": "f64",
                                "action": "parse_sql_numeric_literal",
                                "hint": "Assurez-vous que le nombre utilise le point (.) comme séparateur décimal et ne contient pas de caractères non numériques."
                            })
                        );
                    }
                };
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
